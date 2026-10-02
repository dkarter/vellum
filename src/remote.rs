//! Opt-in, cursor-based sources. Legacy sources keep their existing runner.
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::PathBuf,
    process::Command,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

mod cache;
use cache::Cached;

use crate::{
    app::App,
    builtins::BuiltinSource,
    config::SourceConfig,
    source::{self, Cancellation, SourceItem},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub items: Vec<SourceItem>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_availability: Option<HashMap<String, bool>>,
}

pub fn fetch(
    config: &SourceConfig,
    filter: &str,
    query: &str,
    cursor: Option<&str>,
    size: usize,
    cancellation: Option<&Cancellation>,
) -> Result<Page> {
    if config.builtin == Some(BuiltinSource::GithubPrs) {
        return crate::github_prs::fetch(filter, query, cursor, size, cancellation);
    }
    let cmd = config
        .cmd
        .as_deref()
        .context("remote source requires a command")?;
    let output = source::command_output_cancellable(
        Command::new("sh")
            .args(["-c", cmd])
            .env("VELLUM_FILTER", filter)
            .env("VELLUM_QUERY", query)
            .env("VELLUM_CURSOR", cursor.unwrap_or_default())
            .env("VELLUM_PAGE_SIZE", size.to_string()),
        "remote source",
        cancellation,
    )?;
    let page: Page =
        serde_json::from_str(&output).context("expected remote JSON {items, next_cursor}")?;
    if page.next_cursor.as_deref() == Some("")
        || (cursor.is_some() && page.next_cursor.as_deref() == cursor)
    {
        bail!("remote source returned an empty or unchanged cursor");
    }
    Ok(page)
}

pub(crate) fn any_page(mut fetch: impl FnMut(Option<&str>) -> Result<Page>) -> Result<bool> {
    let mut cursor = None;
    let mut seen = HashSet::new();
    loop {
        let page = fetch(cursor.as_deref())?;
        if !page.items.is_empty() {
            return Ok(true);
        }
        let Some(next) = page.next_cursor else {
            return Ok(false);
        };
        if !seen.insert(next.clone()) {
            bail!("remote source repeated a cursor");
        }
        cursor = Some(next);
    }
}

struct Worker {
    task: crate::worker::Worker<Page>,
    append: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct Controller {
    config: SourceConfig,
    value_field: String,
    filter: Option<String>,
    page: Page,
    fetched_ms: u64,
    worker: Option<Worker>,
    cache_root: Option<PathBuf>,
    scope: String,
    initialized: bool,
    loaded: bool,
    retry_append: Option<bool>,
    cache_writer: Option<cache::Writer>,
    query_input: String,
    server_query: String,
    debounce_until: Option<Instant>,
    baseline: Option<Page>,
    baseline_fetched_ms: u64,
    local_items: Vec<SourceItem>,
    availability: HashMap<String, bool>,
    probes: Vec<(String, crate::worker::Worker<bool>)>,
    availability_initialized: bool,
    availability_fetched_ms: u64,
    availability_failed: bool,
    availability_refresh_at: Option<Instant>,
}

impl Controller {
    pub fn new(config: SourceConfig, value_field: String) -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        let mut scope = format!("v4:{cwd:?}:{config:?}");
        let mut cache_allowed = true;
        if config.builtin == Some(BuiltinSource::GithubPrs)
            && config
                .remote
                .as_ref()
                .is_some_and(|remote| remote.cache_ttl_ms > 0)
        {
            // Read only the public account name, never tokens or resolved auth configuration.
            // Environment-based authentication has no reliable offline account identity.
            cache_allowed = std::env::var_os("GH_TOKEN").is_none()
                && std::env::var_os("GITHUB_TOKEN").is_none()
                && std::env::var_os("GH_ENTERPRISE_TOKEN").is_none()
                && std::env::var_os("GITHUB_ENTERPRISE_TOKEN").is_none();
            let host = std::env::var("GH_HOST").unwrap_or_else(|_| "github.com".into());
            let user = Command::new("gh")
                .args(["config", "get", "user", "--host", &host])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok());
            cache_allowed &= user.as_ref().is_some_and(|user| !user.trim().is_empty());
            scope.push_str(&format!(
                ":{host}:{}:{:?}",
                user.unwrap_or_default().trim(),
                std::env::var_os("GH_REPO")
            ));
        }
        let cache_root = if cache_allowed
            && config
                .remote
                .as_ref()
                .is_some_and(|remote| remote.cache_ttl_ms > 0)
        {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
                .map(|root| root.join("vellum/sources"))
        } else {
            None
        };
        Self {
            config,
            value_field,
            filter: None,
            page: Page::default(),
            fetched_ms: 0,
            worker: None,
            cache_root,
            scope,
            initialized: false,
            loaded: false,
            retry_append: None,
            cache_writer: None,
            query_input: String::new(),
            server_query: String::new(),
            debounce_until: None,
            baseline: None,
            baseline_fetched_ms: 0,
            local_items: Vec::new(),
            availability: HashMap::new(),
            probes: Vec::new(),
            availability_initialized: false,
            availability_fetched_ms: 0,
            availability_failed: false,
            availability_refresh_at: None,
        }
    }

    pub fn pending(&self) -> bool {
        self.worker.is_some() || self.debounce_until.is_some() || !self.probes.is_empty()
    }

    pub fn wake_in(&self) -> Option<Duration> {
        self.debounce_until
            .into_iter()
            .chain(
                self.availability_refresh_at
                    .filter(|_| self.probes.is_empty()),
            )
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .min()
    }

    fn cache_path(&self) -> Option<PathBuf> {
        self.cache_path_for(
            self.filter.as_deref().unwrap_or_default(),
            &self.server_query,
        )
    }

    fn cache_path_for(&self, filter: &str, query: &str) -> Option<PathBuf> {
        let mut hash = DefaultHasher::new();
        self.scope.hash(&mut hash);
        filter.hash(&mut hash);
        query.hash(&mut hash);
        Some(
            self.cache_root
                .as_ref()?
                .join(format!("{:016x}.json", hash.finish())),
        )
    }

    fn read_cache(&self) -> Option<Cached> {
        cache::read(&self.cache_path()?)
    }

    fn write_cache(&mut self) {
        if !cache::fits_page(&self.page) {
            return;
        }
        let Some(path) = self.cache_path() else {
            return;
        };
        let snapshot = Cached {
            fetched_ms: self.fetched_ms,
            page: self.page.clone(),
        };
        self.cache_writer
            .get_or_insert_with(cache::Writer::new)
            .submit(path, snapshot);
    }

    fn start(&mut self, app: &mut App, append: bool) {
        let config = self.config.clone();
        let filter = self.filter.clone().unwrap_or_default();
        let query = self.server_query.clone();
        let cursor = if append {
            self.page.next_cursor.clone()
        } else {
            None
        };
        let size = config.remote.as_ref().expect("remote config").page_size;
        self.retry_append = None;
        let task = crate::worker::Worker::spawn(move |cancellation| {
            fetch(
                &config,
                &filter,
                &query,
                cursor.as_deref(),
                size,
                Some(cancellation),
            )
        });
        self.worker = Some(Worker { task, append });
        app.set_source_status(
            Some(
                if !self.server_query.is_empty() && !append {
                    if self.config.builtin == Some(BuiltinSource::GithubPrs) {
                        "searching GitHub..."
                    } else {
                        "searching source..."
                    }
                } else if append {
                    "loading more..."
                } else if self.loaded {
                    "refreshing source..."
                } else {
                    "loading source..."
                }
                .into(),
            ),
            false,
        );
    }

    pub fn refresh(&mut self, app: &mut App) {
        self.cancel();
        self.availability_initialized = false;
        for (_, probe) in self.probes.drain(..) {
            probe.cancel_in_background();
        }
        self.start(app, false);
    }

    fn cancel(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.task.cancel_in_background();
        }
    }

    fn ttl(&self) -> u64 {
        self.config
            .remote
            .as_ref()
            .expect("remote config")
            .cache_ttl_ms
    }

    fn update_availability(&mut self, app: &mut App) -> bool {
        if !self
            .config
            .remote
            .as_ref()
            .expect("remote config")
            .probe_filters
        {
            return false;
        }
        let mut changed = false;
        let expired = self.probes.is_empty()
            && self
                .availability_refresh_at
                .is_some_and(|deadline| Instant::now() >= deadline);
        if expired {
            self.availability_initialized = false;
            self.availability_refresh_at = None;
        }
        if !self.availability_initialized {
            self.availability_initialized = true;
            self.availability_failed = false;
            let path = self.cache_path_for("@filter-availability", "");
            let cached = path.as_ref().and_then(|path| cache::read(path));
            let fresh = !expired
                && cached
                    .as_ref()
                    .is_some_and(|cache| cache.fresh(now_ms(), self.ttl()));
            if let Some(cache) = cached {
                self.availability = cache.page.filter_availability.unwrap_or_default();
                self.availability_fetched_ms = cache.fetched_ms;
            }
            if fresh {
                let remaining = self
                    .ttl()
                    .saturating_sub(now_ms().saturating_sub(self.availability_fetched_ms));
                self.availability_refresh_at =
                    Some(Instant::now() + Duration::from_millis(remaining));
            }
            for filter in app.filter_values() {
                if fresh && self.availability.contains_key(&filter) {
                    continue;
                }
                let config = self.config.clone();
                let value = filter.clone();
                let probe = crate::worker::Worker::spawn(move |cancellation| {
                    let size = config.remote.as_ref().expect("remote config").page_size;
                    if config.builtin == Some(BuiltinSource::GithubPrs) {
                        return crate::github_prs::has_items(&value, size, Some(cancellation));
                    }
                    any_page(|cursor| fetch(&config, &value, "", cursor, size, Some(cancellation)))
                });
                self.probes.push((filter, probe));
            }
            changed |= app.set_remote_filter_availability(self.availability.clone());
        }
        let mut index = 0;
        let mut completed = false;
        let mut finished = false;
        while index < self.probes.len() {
            let result = self.probes[index].1.try_recv();
            if matches!(result, Err(TryRecvError::Empty)) {
                index += 1;
                continue;
            }
            let (filter, _) = self.probes.swap_remove(index);
            finished = true;
            if let Ok(Ok(available)) = result {
                self.availability.insert(filter, available);
                completed = true;
            } else {
                self.availability_failed = true;
            }
        }
        if completed {
            changed |= app.set_remote_filter_availability(self.availability.clone());
        }
        if finished && self.probes.is_empty() {
            if self.availability_failed {
                self.availability_refresh_at = Some(Instant::now() + Duration::from_secs(30));
            } else {
                self.availability_fetched_ms = now_ms();
                self.availability_refresh_at =
                    (self.ttl() > 0).then(|| Instant::now() + Duration::from_millis(self.ttl()));
                if let Some(path) = self.cache_path_for("@filter-availability", "") {
                    let cache = Cached {
                        fetched_ms: self.availability_fetched_ms,
                        page: Page {
                            filter_availability: Some(self.availability.clone()),
                            ..Page::default()
                        },
                    };
                    self.cache_writer
                        .get_or_insert_with(cache::Writer::new)
                        .submit(path, cache);
                }
            }
        }
        changed
    }

    fn load_scope(&mut self, app: &mut App, elapsed_ms: u64) {
        self.page = Page::default();
        self.loaded = false;
        let mut fresh = false;
        if self.server_query.is_empty()
            && let Some(baseline) = self.baseline.take()
        {
            self.page = baseline;
            self.fetched_ms = self.baseline_fetched_ms;
            self.loaded = true;
            fresh = self.ttl() == 0
                || (Cached {
                    fetched_ms: self.fetched_ms,
                    page: Page::default(),
                })
                .fresh(now_ms(), self.ttl());
        } else if let Some(cache) = self.read_cache() {
            fresh = cache.fresh(now_ms(), self.ttl());
            self.page = cache.page;
            self.fetched_ms = cache.fetched_ms;
            self.loaded = true;
            if self.server_query.is_empty() {
                self.baseline_fetched_ms = self.fetched_ms;
            }
        }
        append_unique(
            &mut self.local_items,
            self.page.items.clone(),
            &self.value_field,
        );
        if let Some(availability) = &self.page.filter_availability {
            for (key, value) in availability {
                self.availability.entry(key.clone()).or_insert(*value);
            }
            app.set_remote_filter_availability(self.availability.clone());
        }
        app.replace_source(self.local_items.clone(), elapsed_ms);
        app.clear_status();
        if !fresh {
            self.start(app, false);
        }
    }

    pub fn update(&mut self, app: &mut App, elapsed_ms: u64) -> bool {
        let filter = app.active_filter().map(|choice| choice.value.clone());
        let more = app.take_load_more_request();
        let mut changed = self.update_availability(app);
        if !self.initialized || self.filter != filter {
            self.cancel();
            self.retry_append = None;
            self.filter = filter;
            self.initialized = true;
            self.baseline = None;
            self.local_items.clear();
            self.server_query.clear();
            self.query_input.clear();
            self.debounce_until = None;
            self.load_scope(app, elapsed_ms);
            changed = true;
        }
        if let Some(debounce_ms) = self
            .config
            .remote
            .as_ref()
            .expect("remote config")
            .search_debounce_ms
        {
            if self.query_input != app.query {
                self.cancel();
                self.retry_append = None;
                self.query_input = app.query.clone();
                app.clear_status();
                if self.query_input.is_empty() {
                    self.debounce_until = None;
                    let was_search = !self.server_query.is_empty();
                    self.server_query.clear();
                    if was_search || !self.loaded {
                        self.load_scope(app, elapsed_ms);
                    }
                } else {
                    self.debounce_until = Some(Instant::now() + Duration::from_millis(debounce_ms));
                }
                changed = true;
            }
            if self
                .debounce_until
                .is_some_and(|until| Instant::now() >= until)
            {
                self.debounce_until = None;
                self.cancel();
                if self.server_query.is_empty() && self.loaded {
                    self.baseline = Some(std::mem::take(&mut self.page));
                    self.baseline_fetched_ms = self.fetched_ms;
                }
                self.server_query = self.query_input.clone();
                self.load_scope(app, elapsed_ms);
                changed = true;
            }
        }
        if let Some(worker) = &self.worker {
            let result = match worker.task.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("remote source worker disconnected")))
                }
            };
            if let Some(result) = result {
                let append = worker.append;
                self.worker = None;
                match result {
                    Ok(page) => {
                        let Page {
                            items,
                            next_cursor,
                            filter_availability,
                        } = page;
                        if append {
                            append_unique(&mut self.page.items, items.clone(), &self.value_field);
                            self.page.next_cursor = next_cursor;
                            if filter_availability.is_some() {
                                self.page.filter_availability = filter_availability.clone();
                            }
                        } else {
                            self.page = Page {
                                items: items.clone(),
                                next_cursor,
                                filter_availability: filter_availability.clone(),
                            };
                            self.fetched_ms = now_ms();
                        }
                        self.loaded = true;
                        if self.server_query.is_empty() {
                            self.baseline = None;
                            self.baseline_fetched_ms = self.fetched_ms;
                        }
                        if self.server_query.is_empty() && !append {
                            self.local_items = items;
                        } else {
                            append_unique(&mut self.local_items, items, &self.value_field);
                        }
                        if let Some(availability) = filter_availability {
                            self.availability.extend(availability);
                            app.set_remote_filter_availability(self.availability.clone());
                        }
                        app.replace_source(self.local_items.clone(), elapsed_ms);
                        app.clear_status();
                        self.write_cache();
                    }
                    Err(error) => {
                        self.retry_append = Some(append);
                        app.set_source_status(Some(format!("source failed: {error:#}")), true)
                    }
                }
                changed = true;
            }
        }
        if more && self.worker.is_none() && self.debounce_until.is_none() {
            if let Some(append) = self.retry_append {
                self.start(app, append);
                changed = true;
            } else if self.loaded && self.page.next_cursor.is_some() {
                self.start(app, true);
                changed = true;
            } else if !self.loaded {
                self.start(app, false);
                changed = true;
            }
        }
        changed
    }
}

fn append_unique(items: &mut Vec<SourceItem>, next: Vec<SourceItem>, value_field: &str) {
    let key = |item: &SourceItem| crate::item::resolve(item, value_field);
    let mut positions: HashMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (key(item), index))
        .collect();
    for item in next {
        let key = key(&item);
        if let Some(&index) = positions.get(&key) {
            items[index] = item;
        } else {
            positions.insert(key, items.len());
            items.push(item);
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel();
        for (_, probe) in self.probes.drain(..) {
            probe.cancel_in_background();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::{fs, thread};

    const CONFIG: &str = "[source]\ncmd = \"printf '%s' '{\\\"items\\\":[],\\\"next_cursor\\\":null}'\"\n[source.remote]\npage_size = 2\n[item]\ntemplate = [['$id']]\nvalue = '$id'";

    fn app(config: &Config, items: Vec<SourceItem>) -> App {
        App::new(
            items,
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        )
    }

    fn settle(controller: &mut Controller, app: &mut App) {
        for _ in 0..400 {
            controller.update(app, 0);
            if !controller.pending() {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("remote source did not finish");
    }

    fn cache_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "vellum-remote-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn seed_cache(controller: &Controller) {
        cache::write(
            &controller.cache_path().unwrap(),
            &Cached {
                fetched_ms: controller.fetched_ms,
                page: controller.page.clone(),
            },
        )
        .unwrap();
    }

    #[test]
    fn rem_007_availability_expires_without_source_refresh() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.remote.as_mut().unwrap().probe_filters = true;
        config.source.remote.as_mut().unwrap().cache_ttl_ms = 300000;
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        controller.cache_root = None;
        let mut app = app(&config, Vec::new());
        settle(&mut controller, &mut app);
        assert_eq!(controller.availability.get(""), Some(&false));
        controller.config.cmd = Some("printf '%s' '{\"items\":[{\"id\":\"new\"}]}'".into());
        controller.availability_refresh_at = Some(Instant::now());
        assert_eq!(controller.wake_in(), Some(Duration::ZERO));
        settle(&mut controller, &mut app);
        assert_eq!(controller.availability.get(""), Some(&true));
    }

    #[test]
    fn rem_008_merge_updates_existing_records_without_reordering() {
        let item = |id: &str, title: &str| {
            serde_json::from_value(serde_json::json!({"id": id, "title": title})).unwrap()
        };
        let mut items = vec![item("a", "old"), item("b", "unchanged")];
        append_unique(
            &mut items,
            vec![item("a", "new"), item("c", "added")],
            "$id",
        );
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["title"], "new");
        assert_eq!(items[1]["id"], "b");
    }

    #[test]
    fn rem_001_navigation_fetches_pages_preserves_selection_and_stops() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.cmd = Some(r#"if [ -z "$VELLUM_CURSOR" ]; then printf '%s' '{"items":[{"id":"a"},{"id":"b"}],"next_cursor":"next"}'; else sleep 0.05; printf '%s' '{"items":[{"id":"b"},{"id":"c"}],"next_cursor":null}'; fi"#.into());
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        let mut app = app(&config, Vec::new());
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        assert_eq!(app.items.len(), 2);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
        controller.update(&mut app, 0);
        assert_eq!(app.status.as_deref(), Some("loading more..."));
        settle(&mut controller, &mut app);
        assert_eq!(app.items.len(), 3);
        assert_eq!(app.selected_item().unwrap().value, "b");
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
        controller.update(&mut app, 0);
        assert!(!controller.pending());
    }

    #[test]
    fn rem_002_filter_switch_cancels_old_result_and_local_search_remains() {
        let mut config = Config::parse(&format!("{CONFIG}\n[input]\nstart_mode = 'filter'\n[filters]\ninitial = 'old'\n[[filters.choices]]\nkey = 'o'\nlabel = 'old'\nsource = 'category'\nvalue = 'old'\n[[filters.choices]]\nkey = 'n'\nlabel = 'new'\nsource = 'category'\nvalue = 'new'")).unwrap();
        config.source.cmd = Some(r#"if [ "$VELLUM_FILTER" = old ]; then sleep 0.2; fi; printf '{"items":[{"id":"%s","category":"%s"}]}' "$VELLUM_FILTER" "$VELLUM_FILTER""#.into());
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        let mut app = app(&config, Vec::new());
        controller.update(&mut app, 0);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('n'),
            crossterm::event::KeyModifiers::NONE,
        ));
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        assert_eq!(app.items.len(), 1);
        assert_eq!(app.selected_item().unwrap().value, "new");
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::NONE,
        ));
        controller.update(&mut app, 0);
        assert!(app.visible.is_empty());
        assert!(!controller.pending());
    }

    #[test]
    fn rem_003_fresh_cache_skips_fetch_stale_cache_renders_then_refreshes() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.remote.as_mut().unwrap().cache_ttl_ms = 300_000;
        config.source.cmd = Some(
            r#"sleep 0.05; printf '%s' '{"items":[{"id":"fresh"}],"next_cursor":null}'"#.into(),
        );
        let root = cache_directory("ttl");
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        controller.cache_root = Some(root.clone());
        controller.page = serde_json::from_value(
            serde_json::json!({"items":[{"id":"cached"}],"next_cursor":"next"}),
        )
        .unwrap();
        controller.fetched_ms = now_ms();
        seed_cache(&controller);
        let mut app = app(&config, Vec::new());
        controller.update(&mut app, 0);
        assert_eq!(app.selected_item().unwrap().value, "cached");
        assert_eq!(controller.page.next_cursor.as_deref(), Some("next"));
        assert!(!controller.pending());

        controller.fetched_ms = now_ms() - 300_001;
        seed_cache(&controller);
        let mut stale = Controller::new(config.source.clone(), config.item.value.clone());
        stale.cache_root = Some(root.clone());
        stale.update(&mut app, 0);
        assert_eq!(app.selected_item().unwrap().value, "cached");
        assert_eq!(app.status.as_deref(), Some("refreshing source..."));
        settle(&mut stale, &mut app);
        assert_eq!(app.selected_item().unwrap().value, "fresh");
        assert!(app.status.is_none());
        stale.cache_writer.take();
        let path = stale.cache_path().unwrap();
        fs::write(path, "broken").unwrap();
        assert!(stale.read_cache().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rem_003_cache_ttl_does_not_slide_on_pagination() {
        let cache = Cached {
            fetched_ms: 100,
            page: Page::default(),
        };
        assert!(cache.fresh(399, 300));
        assert!(!cache.fresh(400, 300));
        assert!(!cache.fresh(99, 300));
        assert!(!cache.fresh(100, 0));
        let config = Config::parse(CONFIG).unwrap();
        let mut controller = Controller::new(config.source, "$id".into());
        controller.cache_root = Some(std::env::temp_dir().join("vellum-missing-cache-test"));
        assert!(controller.read_cache().is_none());
    }

    #[test]
    fn rem_001_appends_unique_values_without_reordering() {
        let item = |id| serde_json::from_value(serde_json::json!({"id": id})).unwrap();
        let mut items = vec![item("a"), item("b")];
        append_unique(&mut items, vec![item("b"), item("c")], "$id");
        assert_eq!(items, vec![item("a"), item("b"), item("c")]);
    }

    #[test]
    fn rem_002_command_receives_filter_cursor_and_size() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.cmd = Some(r#"printf '{"items":[{"filter":"%s","cursor":"%s","size":"%s"}],"next_cursor":"next"}' "$VELLUM_FILTER" "$VELLUM_CURSOR" "$VELLUM_PAGE_SIZE""#.into());
        let page = fetch(&config.source, "mine-open", "", Some("old"), 2, None).unwrap();
        assert_eq!(page.items[0]["filter"], "mine-open");
        assert_eq!(page.items[0]["cursor"], "old");
        assert_eq!(page.items[0]["size"], "2");
        config.source.cmd = Some("printf '%s' '{\"items\":[],\"next_cursor\":\"old\"}'".into());
        assert!(fetch(&config.source, "", "", Some("old"), 2, None).is_err());
    }

    #[test]
    fn rem_004_failure_preserves_items_and_reports_error() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.cmd = Some("exit 1".into());
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        let mut app = App::new(
            Vec::new(),
            config.item,
            config.keybindings,
            config.filters,
            config.input,
            true,
        );
        controller.update(&mut app, 0);
        for _ in 0..200 {
            if controller.update(&mut app, 0) && !controller.pending() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.status_is_error());
        assert!(app.status.unwrap().starts_with("source failed:"));
    }

    #[test]
    fn rem_004_failed_refresh_retries_first_page_not_cached_cursor() {
        for cursor in [None, Some("old")] {
            let mut config = Config::parse(CONFIG).unwrap();
            config.source.cmd = Some(format!(
                "printf '%s' '{}'",
                serde_json::json!({"items":[{"id":"kept"}],"next_cursor":cursor})
            ));
            let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
            let mut app = app(&config, Vec::new());
            controller.update(&mut app, 0);
            settle(&mut controller, &mut app);
            controller.config.cmd = Some("exit 1".into());
            controller.refresh(&mut app);
            settle(&mut controller, &mut app);
            assert_eq!(app.selected_item().unwrap().value, "kept");
            assert!(app.status_is_error());
            controller.config.cmd =
                Some(r#"printf '{"items":[{"id":"fresh%s"}]}' "$VELLUM_CURSOR""#.into());
            app.handle_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Down,
                crossterm::event::KeyModifiers::NONE,
            ));
            controller.update(&mut app, 0);
            settle(&mut controller, &mut app);
            assert_eq!(app.items.len(), 1);
            assert_eq!(app.selected_item().unwrap().value, "fresh");
        }
    }

    #[test]
    fn rem_007_default_availability_dims_skips_and_keeps_shortcuts_and_initial_filter() {
        let mut config = Config::parse(&format!("{CONFIG}\n[input]\nstart_mode = 'filter'\n[filters]\ninitial = 'empty'\n[[filters.choices]]\nkey = 'e'\nlabel = 'empty'\nsource = 'category'\nvalue = 'empty'\n[[filters.choices]]\nkey = 'n'\nlabel = 'nonempty'\nsource = 'category'\nvalue = 'nonempty'\n[[filters.choices]]\nkey = 'z'\nlabel = 'zero'\nsource = 'category'\nvalue = 'zero'")).unwrap();
        config.source.remote.as_mut().unwrap().probe_filters = true;
        config.source.remote.as_mut().unwrap().cache_ttl_ms = 300_000;
        config.source.cmd = Some(r#"case "$VELLUM_FILTER:$VELLUM_CURSOR" in empty:) printf '%s' '{"items":[],"next_cursor":"last"}';; empty:last|zero:*) printf '%s' '{"items":[]}';; *) printf '{"items":[{"id":"one","category":"%s"}]}' "$VELLUM_FILTER";; esac"#.into());
        let root = cache_directory("availability");
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        controller.cache_root = Some(root.clone());
        let mut app = app(&config, Vec::new());
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        assert_eq!(app.active_filter().unwrap().value, "empty");
        assert!(!app.filter_has_items(Some(0)));
        assert!(app.filter_has_items(Some(1)));
        assert!(!app.filter_has_items(Some(2)));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Tab,
        ));
        assert_eq!(app.active_filter().unwrap().value, "nonempty");
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('z'),
        ));
        assert_eq!(app.active_filter().unwrap().value, "zero");
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Esc,
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('x'),
        ));
        assert!(app.visible.is_empty());
        assert!(
            app.filter_has_items(Some(1)),
            "default-category availability must not depend on the local query"
        );
        controller.cache_writer.take();
        let mut cached = Controller::new(config.source.clone(), config.item.value.clone());
        cached.cache_root = Some(root.clone());
        let mut cached_app = super::tests::app(&config, Vec::new());
        cached.update(&mut cached_app, 0);
        assert!(
            !cached.pending(),
            "fresh default counts and source pages must not refetch"
        );
        assert_eq!(cached.availability.get("empty"), Some(&false));
        assert_eq!(cached.availability.get("nonempty"), Some(&true));
        drop(cached);
        drop(controller);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rem_008_local_search_is_immediate_server_search_debounces_and_cancels() {
        let mut config = Config::parse(CONFIG).unwrap();
        config.source.remote.as_mut().unwrap().search_debounce_ms = Some(300);
        config.source.cmd = Some(r#"case "$VELLUM_QUERY:$VELLUM_CURSOR" in :) printf '%s' '{"items":[{"id":"alpha-local"},{"id":"other-local"}],"next_cursor":"base"}';; :base) printf '%s' '{"items":[{"id":"last-local"}]}';; alpha:) sleep 0.05; printf '%s' '{"items":[{"id":"alpha-server"},{"id":"alpha-local"}],"next_cursor":"remote"}';; beta:) printf '%s' '{"items":[{"id":"beta-server"}]}';; *) sleep 0.2; printf '%s' '{"items":[{"id":"obsolete"}]}';; esac"#.into());
        let mut controller = Controller::new(config.source.clone(), config.item.value.clone());
        let mut app = app(&config, Vec::new());
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        let baseline_time = controller.fetched_ms;
        for ch in "alpha".chars() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(ch),
            ));
        }
        controller.update(&mut app, 0);
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_item().unwrap().value, "alpha-local");
        assert!(
            controller.worker.is_none(),
            "typing should not launch a request before the debounce"
        );
        controller.debounce_until = Some(Instant::now());
        controller.update(&mut app, 0);
        assert_eq!(app.status.as_deref(), Some("searching source..."));
        settle(&mut controller, &mut app);
        assert_eq!(app.query, "alpha");
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.selected_item().unwrap().value, "alpha-local");
        assert_eq!(controller.page.next_cursor.as_deref(), Some("remote"));

        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('x'),
        ));
        controller.update(&mut app, 0);
        controller.debounce_until = Some(Instant::now());
        controller.update(&mut app, 0);
        for _ in 0..6 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Backspace,
            ));
        }
        for ch in "beta".chars() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(ch),
            ));
        }
        controller.update(&mut app, 0);
        assert!(app.visible.is_empty());
        assert!(controller.worker.is_none());
        controller.debounce_until = Some(Instant::now());
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        assert_eq!(app.selected_item().unwrap().value, "beta-server");
        assert!(!app.items.iter().any(|item| item.value == "obsolete"));
        for _ in 0..4 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Backspace,
            ));
        }
        controller.update(&mut app, 0);
        assert_eq!(controller.page.next_cursor.as_deref(), Some("base"));
        assert_eq!(
            controller.fetched_ms, baseline_time,
            "search must not renew the default cache TTL"
        );
        assert!(!controller.pending());
        assert_eq!(app.selected_item().unwrap().value, "alpha-local");
        for _ in 1..app.visible.len() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Down,
            ));
        }
        controller.update(&mut app, 0);
        settle(&mut controller, &mut app);
        assert!(app.items.iter().any(|item| item.value == "last-local"));
        assert_eq!(app.selected_item().unwrap().value, "beta-server");
    }
}
