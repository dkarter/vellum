use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str, pattern::Pattern};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    action::{AvailabilityCommand, prepare_availability},
    config::{
        ActionsConfig, Bindings, FilterChoice, FilterConfig, InputConfig, InputMode, ItemConfig,
        Keybindings,
    },
    frecency::FrecencyRank,
    item::{RenderedItem, field_value_at, matching_indices_with_frecency_by, render_items},
    source::SourceItem,
};

#[derive(Debug)]
struct CachedAvailability {
    available: bool,
    checked_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionAvailabilityState {
    Available,
    Pending,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Accepted(String),
    ActionRequested(usize),
    ActionCompleted,
    Cancelled,
}

#[derive(Debug)]
pub struct App {
    item_config: ItemConfig,
    keybindings: Keybindings,
    filter_config: FilterConfig,
    input_config: InputConfig,
    action_config: ActionsConfig,
    availability_cache: HashMap<AvailabilityCommand, CachedAvailability>,
    availability_in_flight: HashSet<AvailabilityCommand>,
    availability_queue: Vec<AvailabilityCommand>,
    pending_action: Option<(usize, SourceItem)>,
    search_enabled: bool,
    source_items: Vec<SourceItem>,
    frecency_scores: HashMap<String, FrecencyRank>,
    pub items: Vec<RenderedItem>,
    pub visible: Vec<usize>,
    pub query: String,
    pub cursor: usize,
    pub input_mode: InputMode,
    pub filter_mode: bool,
    pub action_menu: bool,
    pub action_selected: usize,
    pub action_query: String,
    pub action_cursor: usize,
    pub status: Option<String>,
    active_filter: Option<usize>,
    pub selected: usize,
    pub outcome: Outcome,
}

impl App {
    pub fn new(
        source_items: Vec<SourceItem>,
        item_config: ItemConfig,
        keybindings: Keybindings,
        filter_config: FilterConfig,
        input_config: InputConfig,
        search_enabled: bool,
    ) -> Self {
        Self::new_with_frecency(
            source_items,
            item_config,
            keybindings,
            filter_config,
            input_config,
            search_enabled,
            HashMap::new(),
        )
    }

    pub fn new_with_frecency(
        source_items: Vec<SourceItem>,
        item_config: ItemConfig,
        keybindings: Keybindings,
        filter_config: FilterConfig,
        input_config: InputConfig,
        search_enabled: bool,
        frecency_scores: HashMap<String, FrecencyRank>,
    ) -> Self {
        Self::new_with_frecency_and_actions(
            source_items,
            item_config,
            keybindings,
            filter_config,
            input_config,
            search_enabled,
            frecency_scores,
            ActionsConfig::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_frecency_and_actions(
        source_items: Vec<SourceItem>,
        item_config: ItemConfig,
        keybindings: Keybindings,
        filter_config: FilterConfig,
        input_config: InputConfig,
        search_enabled: bool,
        frecency_scores: HashMap<String, FrecencyRank>,
        action_config: ActionsConfig,
    ) -> Self {
        let items = render_items(&source_items, &item_config, 0);
        let mut app = Self {
            item_config,
            keybindings,
            filter_config,
            action_config,
            availability_cache: HashMap::new(),
            availability_in_flight: HashSet::new(),
            availability_queue: Vec::new(),
            pending_action: None,
            search_enabled,
            source_items,
            frecency_scores,
            items,
            visible: Vec::new(),
            query: String::new(),
            cursor: 0,
            input_mode: if input_config.vim {
                input_config.start_mode
            } else {
                InputMode::Insert
            },
            input_config,
            filter_mode: false,
            action_menu: false,
            action_selected: 0,
            action_query: String::new(),
            action_cursor: 0,
            status: None,
            active_filter: None,
            selected: 0,
            outcome: Outcome::Running,
        };
        app.visible = app.matching_indices();
        app
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&'c'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.outcome = Outcome::Cancelled;
            return;
        }
        if self.action_menu {
            if key.code == KeyCode::Esc {
                self.action_menu = false;
            } else if self.bindings_match(key, &self.keybindings.down) {
                self.move_action_down();
            } else if self.bindings_match(key, &self.keybindings.up) {
                self.action_selected = self.action_selected.saturating_sub(1);
            } else if self.bindings_match(key, &self.keybindings.accept) {
                if let Some(index) = self
                    .matching_action_indices()
                    .get(self.action_selected)
                    .copied()
                {
                    self.action_menu = false;
                    self.request_action(index);
                }
            } else {
                self.handle_action_query_key(key);
            }
            return;
        }
        if self.filter_mode {
            if key.code == KeyCode::Esc || self.filter_config.mode.matches(key) {
                self.filter_mode = false;
            } else if self.bindings_match(key, &self.keybindings.down) {
                self.move_down();
            } else if self.bindings_match(key, &self.keybindings.up) {
                self.move_up();
            } else if self.filter_config.clear.matches(key) {
                if self.active_filter.take().is_some() {
                    self.refilter();
                }
            } else if let Some(index) = self
                .filter_config
                .choices
                .iter()
                .position(|choice| choice.key.matches(key))
            {
                self.active_filter = (self.active_filter != Some(index)).then_some(index);
                self.refilter();
            } else if self.bindings_match(key, &self.keybindings.accept) {
                self.filter_mode = false;
                self.accept_selected();
            }
            return;
        }
        if self.keybindings.enabled
            && let Some(index) = self
                .action_config
                .items
                .iter()
                .enumerate()
                .find(|(index, action)| {
                    action.key.matches(key) && self.action_statically_available(*index)
                })
                .map(|(index, _)| index)
        {
            self.request_action(index);
            return;
        }
        if !self.filter_config.choices.is_empty() && self.filter_config.mode.matches(key) {
            self.filter_mode = true;
            return;
        }
        if self.keybindings.enabled && self.action_config.menu.matches(key) {
            if !self.has_potential_actions() {
                return;
            }
            self.queue_availability_checks();
            let available_actions = self.available_action_indices();
            self.action_selected = self
                .action_config
                .default_index()
                .and_then(|default| available_actions.iter().position(|index| *index == default))
                .unwrap_or(0);
            self.action_query.clear();
            self.action_cursor = 0;
            self.action_menu = true;
            return;
        }
        if key.code == KeyCode::Esc && self.input_config.vim {
            if self.input_mode == InputMode::Insert {
                self.input_mode = InputMode::Normal;
                self.cursor = previous_boundary(&self.query, self.cursor);
            } else {
                self.outcome = Outcome::Cancelled;
            }
            return;
        }
        if self.bindings_match(key, &self.keybindings.cancel) {
            self.outcome = Outcome::Cancelled;
        } else if self.bindings_match(key, &self.keybindings.accept) {
            self.accept_selected();
        } else if self.bindings_match(key, &self.keybindings.down) {
            self.move_down();
        } else if self.bindings_match(key, &self.keybindings.up) {
            self.move_up();
        } else if self.search_enabled {
            if self.input_mode == InputMode::Normal {
                self.handle_normal_key(key);
            } else {
                self.handle_insert_key(key);
            }
        }
    }

    pub fn vim_enabled(&self) -> bool {
        self.input_config.vim
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        self.items = render_items(&self.source_items, &self.item_config, elapsed_ms);
        self.visible = self.matching_indices();
        self.clamp_selection();
    }

    pub fn animation_interval(&self) -> Option<std::time::Duration> {
        self.item_config
            .tokens
            .iter()
            .filter_map(|token| token.animation_fps)
            .max()
            .map(|fps| std::time::Duration::from_millis(1_000 / u64::from(fps)))
    }

    pub fn replace_source(&mut self, source_items: Vec<SourceItem>, elapsed_ms: u64) -> bool {
        if self.source_items == source_items {
            return false;
        }
        let selected_value = self.selected_item().map(|item| item.value.clone());
        let selected_action = self
            .action_menu
            .then(|| {
                self.matching_action_indices()
                    .get(self.action_selected)
                    .copied()
            })
            .flatten();
        self.source_items = source_items;
        self.items = render_items(&self.source_items, &self.item_config, elapsed_ms);
        self.visible = self.matching_indices();
        let previous_position = self.selected;
        let restored_position = selected_value.as_ref().and_then(|value| {
            self.visible
                .iter()
                .position(|&index| self.items[index].value == *value)
        });
        self.selected = restored_position.unwrap_or(previous_position);
        self.clamp_selection();
        if self.action_menu {
            let matching = self.matching_action_indices();
            let restored_action = selected_action
                .and_then(|action| matching.iter().position(|index| *index == action));
            if restored_position.is_none() {
                self.action_menu = false;
            } else if selected_action.is_none() {
                self.action_selected = 0;
            } else if let Some(position) = restored_action {
                self.action_selected = position;
            } else if selected_action.is_some_and(|index| {
                self.action_availability(index) == ActionAvailabilityState::Pending
            }) {
                self.action_selected = self.action_selected.min(matching.len().saturating_sub(1));
            } else {
                self.action_menu = false;
            }
        }
        true
    }

    pub fn selected_item(&self) -> Option<&RenderedItem> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.items.get(index))
    }

    pub fn selected_source_item(&self) -> Option<&SourceItem> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.source_items.get(index))
    }

    pub fn finish_action(&mut self, error: Option<String>) {
        self.outcome = Outcome::Running;
        self.status = error;
    }

    pub fn take_availability_checks(&mut self) -> Vec<AvailabilityCommand> {
        if self.action_menu || self.pending_action.is_some() {
            self.queue_availability_checks();
        }
        std::mem::take(&mut self.availability_queue)
    }

    pub fn finish_availability_check(&mut self, check: AvailabilityCommand, available: bool) {
        let selected_action = self
            .action_menu
            .then(|| {
                self.matching_action_indices()
                    .get(self.action_selected)
                    .copied()
            })
            .flatten();
        self.availability_in_flight.remove(&check);
        self.availability_cache.insert(
            check,
            CachedAvailability {
                available,
                checked_at: Instant::now(),
            },
        );
        let matching = self.matching_action_indices();
        self.action_selected = selected_action
            .and_then(|index| matching.iter().position(|candidate| *candidate == index))
            .unwrap_or_else(|| self.action_selected.min(matching.len().saturating_sub(1)));

        if let Some((index, item)) = self.pending_action.clone() {
            if self.selected_source_item() != Some(&item) {
                self.pending_action = None;
            } else {
                match self.action_availability(index) {
                    ActionAvailabilityState::Available => {
                        self.pending_action = None;
                        self.outcome = Outcome::ActionRequested(index);
                    }
                    ActionAvailabilityState::Unavailable => self.pending_action = None,
                    ActionAvailabilityState::Pending => {}
                }
            }
        }
    }

    pub fn invalidate_availability(&mut self) {
        self.availability_cache.clear();
        self.availability_in_flight.clear();
        self.availability_queue.clear();
        self.pending_action = None;
    }

    pub fn available_action_indices(&self) -> Vec<usize> {
        self.action_config
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                (self.action_availability(index) == ActionAvailabilityState::Available)
                    .then_some(index)
            })
            .collect()
    }

    pub fn has_potential_actions(&self) -> bool {
        self.action_config
            .items
            .iter()
            .enumerate()
            .any(|(index, _)| {
                self.action_availability(index) != ActionAvailabilityState::Unavailable
            })
    }

    pub fn has_pending_actions(&self) -> bool {
        self.action_config
            .items
            .iter()
            .enumerate()
            .any(|(index, _)| self.action_availability(index) == ActionAvailabilityState::Pending)
    }

    pub fn availability_refresh_in(&self) -> Option<Duration> {
        self.action_menu.then_some(())?;
        self.action_config
            .items
            .iter()
            .filter_map(|action| {
                let item = self.selected_source_item()?;
                action.is_available(item).then_some(())?;
                let availability = action.availability.as_ref()?;
                let check = prepare_availability(availability, item).ok()?;
                (!self.availability_in_flight.contains(&check)).then_some(())?;
                let cached = self.availability_cache.get(&check)?;
                Some(
                    Duration::from_millis(availability.cache_ms)
                        .saturating_sub(cached.checked_at.elapsed()),
                )
            })
            .min()
    }

    pub fn matching_action_indices(&self) -> Vec<usize> {
        let available = self.available_action_indices();
        if self.action_query.is_empty() {
            return available;
        }
        let pattern = Pattern::parse(
            &self.action_query,
            nucleo_matcher::pattern::CaseMatching::Smart,
            nucleo_matcher::pattern::Normalization::Smart,
        );
        let mut matcher = Matcher::new(MatcherConfig::DEFAULT);
        let mut buffer = Vec::new();
        let mut matches: Vec<_> = available
            .into_iter()
            .filter_map(|index| {
                let action = &self.action_config.items[index];
                let text = format!("{} {} {}", action.name, action.label, action.description);
                pattern
                    .score(Utf32Str::new(&text, &mut buffer), &mut matcher)
                    .map(|score| (index, score))
            })
            .collect();
        matches.sort_unstable_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        matches.into_iter().map(|(index, _)| index).collect()
    }

    pub fn active_filter(&self) -> Option<&FilterChoice> {
        self.active_filter
            .and_then(|index| self.filter_config.choices.get(index))
    }

    fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.selected = (self.selected + 1).min(self.visible.len() - 1);
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_action_down(&mut self) {
        self.action_selected =
            (self.action_selected + 1).min(self.matching_action_indices().len().saturating_sub(1));
    }

    fn handle_action_query_key(&mut self, key: KeyEvent) {
        let changed = match key.code {
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.action_query.insert(self.action_cursor, character);
                self.action_cursor += character.len_utf8();
                true
            }
            KeyCode::Backspace if self.action_cursor > 0 => {
                let start = previous_boundary(&self.action_query, self.action_cursor);
                self.action_query
                    .replace_range(start..self.action_cursor, "");
                self.action_cursor = start;
                true
            }
            KeyCode::Delete if self.action_cursor < self.action_query.len() => {
                let end = next_boundary(&self.action_query, self.action_cursor);
                self.action_query.replace_range(self.action_cursor..end, "");
                true
            }
            KeyCode::Left => {
                self.action_cursor = previous_boundary(&self.action_query, self.action_cursor);
                false
            }
            KeyCode::Right => {
                self.action_cursor = next_boundary(&self.action_query, self.action_cursor);
                false
            }
            _ => false,
        };
        if changed {
            self.action_selected = 0;
        }
    }

    fn request_action(&mut self, index: usize) {
        match self.action_availability(index) {
            ActionAvailabilityState::Available => self.outcome = Outcome::ActionRequested(index),
            ActionAvailabilityState::Pending => {
                if let Some(item) = self.selected_source_item().cloned() {
                    self.pending_action = Some((index, item));
                    self.queue_availability_checks();
                }
            }
            ActionAvailabilityState::Unavailable => {}
        }
    }

    fn accept_selected(&mut self) {
        if let Some(index) = self.action_config.default_index() {
            self.request_action(index);
        } else if let Some(item) = self.selected_item() {
            self.outcome = Outcome::Accepted(item.value.clone());
        }
    }

    fn action_statically_available(&self, index: usize) -> bool {
        self.action_config
            .items
            .get(index)
            .zip(self.selected_source_item())
            .is_some_and(|(action, item)| action.is_available(item))
    }

    fn action_availability(&self, index: usize) -> ActionAvailabilityState {
        let Some((action, item)) = self
            .action_config
            .items
            .get(index)
            .zip(self.selected_source_item())
        else {
            return ActionAvailabilityState::Unavailable;
        };
        if !action.is_available(item) {
            return ActionAvailabilityState::Unavailable;
        }
        let Some(availability) = &action.availability else {
            return ActionAvailabilityState::Available;
        };
        let Ok(check) = prepare_availability(availability, item) else {
            return ActionAvailabilityState::Unavailable;
        };
        match self.cached_availability(&check, availability.cache_ms) {
            Some(true) => ActionAvailabilityState::Available,
            Some(false) => ActionAvailabilityState::Unavailable,
            None => ActionAvailabilityState::Pending,
        }
    }

    fn queue_availability_checks(&mut self) {
        let active_checks = self
            .selected_source_item()
            .into_iter()
            .flat_map(|item| {
                self.action_config.items.iter().filter_map(|action| {
                    action
                        .is_available(item)
                        .then_some(action.availability.as_ref())
                        .flatten()
                        .and_then(|availability| prepare_availability(availability, item).ok())
                })
            })
            .collect::<HashSet<_>>();
        while self.availability_cache.len() > 256 {
            let Some(oldest) = self
                .availability_cache
                .iter()
                .filter(|(check, _)| !active_checks.contains(*check))
                .min_by_key(|(_, cached)| cached.checked_at)
                .map(|(check, _)| check.clone())
            else {
                break;
            };
            self.availability_cache.remove(&oldest);
        }
        let checks = self
            .selected_source_item()
            .into_iter()
            .flat_map(|item| {
                self.action_config
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, action)| {
                        (self.action_availability(index) == ActionAvailabilityState::Pending)
                            .then_some(action.availability.as_ref())
                            .flatten()
                            .and_then(|availability| {
                                prepare_availability(availability, item)
                                    .ok()
                                    .map(|check| (check, availability.cache_ms))
                            })
                    })
            })
            .collect::<Vec<_>>();
        for (check, cache_ms) in checks {
            if self.cached_availability(&check, cache_ms).is_none()
                && self.availability_in_flight.insert(check.clone())
            {
                self.availability_queue.push(check);
            }
        }
    }

    fn cached_availability(&self, check: &AvailabilityCommand, cache_ms: u64) -> Option<bool> {
        self.availability_cache.get(check).and_then(|cached| {
            (cached.checked_at.elapsed() < Duration::from_millis(cache_ms))
                .then_some(cached.available)
        })
    }

    fn refilter(&mut self) {
        self.visible = self.matching_indices();
        self.selected = 0;
    }

    fn matching_indices(&self) -> Vec<usize> {
        let choice = self.active_filter();
        let path = choice.map(|choice| {
            choice
                .source
                .strip_prefix('$')
                .unwrap_or(&choice.source)
                .split('.')
                .collect::<Vec<_>>()
        });
        matching_indices_with_frecency_by(
            &self.items,
            &self.query,
            &self.frecency_scores,
            |index| {
                choice.is_none_or(|choice| {
                    field_value_at(
                        &self.source_items[index],
                        path.as_deref().unwrap_or_default(),
                    )
                    .and_then(serde_json::Value::as_str)
                        == Some(&choice.value)
                })
            },
        )
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        if self.bindings_match(key, &self.keybindings.forward) {
            self.cursor = next_boundary(&self.query, self.cursor);
        } else if self.bindings_match(key, &self.keybindings.backward) {
            self.cursor = previous_boundary(&self.query, self.cursor);
        } else if self.bindings_match(key, &self.keybindings.start) {
            self.cursor = 0;
        } else if self.bindings_match(key, &self.keybindings.end) {
            self.cursor = self.query.len();
        } else if self.bindings_match(key, &self.keybindings.delete_word) {
            self.delete_previous_word();
        } else {
            match key.code {
                KeyCode::Char(character)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.query.insert(self.cursor, character);
                    self.cursor += character.len_utf8();
                    self.refilter();
                }
                KeyCode::Backspace if self.cursor > 0 => {
                    let start = previous_boundary(&self.query, self.cursor);
                    self.query.replace_range(start..self.cursor, "");
                    self.cursor = start;
                    self.refilter();
                }
                KeyCode::Delete if self.cursor < self.query.len() => {
                    self.delete_at_cursor();
                }
                _ => {}
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.modifiers != KeyModifiers::NONE && key.modifiers != KeyModifiers::SHIFT {
            return;
        }
        match key.code {
            KeyCode::Char('i') => self.input_mode = InputMode::Insert,
            KeyCode::Char('a') => {
                self.cursor = next_boundary(&self.query, self.cursor);
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('I') => {
                self.cursor = 0;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('A') => {
                self.cursor = self.query.len();
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.cursor = previous_boundary(&self.query, self.cursor);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let next = next_boundary(&self.query, self.cursor);
                if next < self.query.len() {
                    self.cursor = next;
                }
            }
            KeyCode::Char('0') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('$') | KeyCode::End => self.cursor = last_boundary(&self.query),
            KeyCode::Char('b') => self.cursor = previous_word_start(&self.query, self.cursor),
            KeyCode::Char('w') => {
                self.cursor =
                    next_word_start(&self.query, self.cursor).min(last_boundary(&self.query));
            }
            KeyCode::Char('x') | KeyCode::Delete => self.delete_at_cursor(),
            KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('k') => self.move_up(),
            _ => {}
        }
    }

    fn delete_previous_word(&mut self) {
        let start = previous_word_start(&self.query, self.cursor);
        self.query.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.refilter();
    }

    fn delete_at_cursor(&mut self) {
        if self.cursor >= self.query.len() {
            return;
        }
        let end = next_boundary(&self.query, self.cursor);
        self.query.replace_range(self.cursor..end, "");
        if self.input_mode == InputMode::Normal && self.cursor == self.query.len() {
            self.cursor = last_boundary(&self.query);
        }
        self.refilter();
    }

    fn bindings_match(&self, key: KeyEvent, bindings: &Bindings) -> bool {
        self.keybindings.enabled && bindings.matches(key)
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
}

fn previous_boundary(value: &str, cursor: usize) -> usize {
    value[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_boundary(value: &str, cursor: usize) -> usize {
    value[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(value.len(), |(index, _)| cursor + index)
}

fn last_boundary(value: &str) -> usize {
    value
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn previous_word_start(value: &str, cursor: usize) -> usize {
    let graphemes: Vec<_> = value[..cursor].grapheme_indices(true).collect();
    let mut position = graphemes.len();
    while position > 0 && graphemes[position - 1].1.chars().all(char::is_whitespace) {
        position -= 1;
    }
    while position > 0 && !graphemes[position - 1].1.chars().all(char::is_whitespace) {
        position -= 1;
    }
    graphemes.get(position).map_or(0, |(index, _)| *index)
}

fn next_word_start(value: &str, cursor: usize) -> usize {
    let graphemes: Vec<_> = value.grapheme_indices(true).collect();
    let mut position = graphemes.partition_point(|(index, _)| *index < cursor);
    while position < graphemes.len() && !graphemes[position].1.chars().all(char::is_whitespace) {
        position += 1;
    }
    while position < graphemes.len() && graphemes[position].1.chars().all(char::is_whitespace) {
        position += 1;
    }
    graphemes
        .get(position)
        .map_or(value.len(), |(index, _)| *index)
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyEvent;
    use serde_json::json;

    use super::*;

    fn app() -> App {
        let config: ItemConfig = toml::from_str(
            r#"
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        let source = json!([
            { "id": "1", "name": "Alpha" },
            { "id": "2", "name": "Beta" }
        ]);
        App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config,
            Keybindings::default(),
            FilterConfig::default(),
            InputConfig::default(),
            true,
        )
    }

    fn app_with_actions() -> App {
        let mut app = app();
        app.action_config = toml::from_str(
            r#"
            default = "open"
            menu = "ctrl-a"

            [[items]]
            name = "open"
            label = "Open"
            icon = "→"
            description = "View this item"
            command = ["tool", "open", "$id"]

            [[items]]
            name = "remove"
            label = "Remove"
            icon = "!"
            description = "Delete this item"
            key = "ctrl-r"
            command = ["tool", "remove", "$id"]
            on_success = "refresh"
            "#,
        )
        .unwrap();
        app
    }

    #[test]
    fn sea_002_filters_navigates_and_accepts() {
        let mut app = app();

        app.handle_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(app.visible, [1]);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::Accepted("2".into()));
    }

    #[test]
    fn act_002_enter_remains_selection_output_without_a_default_action() {
        let mut app = app();

        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert_eq!(app.outcome, Outcome::Accepted("1".into()));
    }

    #[test]
    fn act_003_default_and_direct_keys_request_actions() {
        let mut app = app_with_actions();

        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::ActionRequested(0));

        app.finish_action(None);
        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(app.outcome, Outcome::ActionRequested(1));
    }

    #[test]
    fn act_004_quick_action_menu_navigates_and_selects() {
        let mut app = app_with_actions();

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert!(app.action_menu);
        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::ActionRequested(1));

        app.finish_action(None);
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.action_menu);
        assert_eq!(app.outcome, Outcome::Running);
    }

    #[test]
    fn act_009_quick_action_menu_fuzzy_filters_metadata() {
        let mut app = app_with_actions();
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        for character in "dlt".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }

        assert_eq!(app.action_query, "dlt");
        assert_eq!(app.matching_action_indices(), [1]);
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.action_selected, 0);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::ActionRequested(1));
    }

    #[test]
    fn act_011_command_gated_actions_resolve_asynchronously_and_share_cached_results() {
        let mut app = app();
        app.action_config = toml::from_str(
            r#"
            menu = "ctrl-a"

            [[items]]
            name = "pr"
            label = "Open PR"
            command = ["true"]
            availability = { command = ["test", "-n", "$id"], cache_ms = 30000 }

            [[items]]
            name = "checks"
            label = "Open checks"
            command = ["true"]
            availability = { command = ["test", "-n", "$id"], cache_ms = 30000 }
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

        assert!(app.action_menu);
        assert!(app.matching_action_indices().is_empty());
        assert!(app.has_pending_actions());
        let checks = app.take_availability_checks();
        assert_eq!(checks.len(), 1);

        app.finish_availability_check(checks[0].clone(), true);

        assert_eq!(app.matching_action_indices(), [0, 1]);
        assert!(!app.has_pending_actions());
        app.action_menu = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert!(app.take_availability_checks().is_empty());

        app.availability_cache
            .get_mut(&checks[0])
            .unwrap()
            .checked_at = Instant::now() - Duration::from_secs(31);
        app.action_menu = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        let checks = app.take_availability_checks();
        assert_eq!(checks.len(), 1);
        app.finish_availability_check(checks[0].clone(), false);
        assert!(app.matching_action_indices().is_empty());
        assert!(!app.has_potential_actions());
    }

    #[test]
    fn act_011_async_results_preserve_the_highlighted_action() {
        let mut app = app();
        app.action_config = toml::from_str(
            r#"
            menu = "ctrl-a"

            [[items]]
            name = "gated"
            label = "Gated"
            command = ["true"]
            availability = { command = ["true"] }

            [[items]]
            name = "visible"
            label = "Visible"
            command = ["true"]
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert_eq!(app.matching_action_indices(), [1]);
        let check = app.take_availability_checks().remove(0);

        app.finish_availability_check(check, true);

        assert_eq!(app.matching_action_indices(), [0, 1]);
        assert_eq!(app.action_selected, 1);
    }

    #[test]
    fn act_003_act_011_direct_gated_action_waits_for_its_probe() {
        let mut app = app();
        app.action_config = toml::from_str(
            r#"
            [[items]]
            name = "gated"
            label = "Gated"
            key = "ctrl-r"
            command = ["true"]
            availability = { command = ["true"] }
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

        assert_eq!(app.outcome, Outcome::Running);
        let check = app.take_availability_checks().remove(0);
        app.finish_availability_check(check, true);
        assert_eq!(app.outcome, Outcome::ActionRequested(0));
    }

    #[test]
    fn act_011_menu_eligibility_checks_every_action() {
        let mut app = app();
        app.action_config = toml::from_str(
            r#"
            menu = "ctrl-a"

            [[items]]
            name = "hidden"
            label = "Hidden"
            command = ["true"]
            when = [{ field = "missing", is_set = true }]

            [[items]]
            name = "visible"
            label = "Visible"
            command = ["true"]
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

        assert!(app.action_menu);
        assert_eq!(app.matching_action_indices(), [1]);
    }

    #[test]
    fn fil_001_filter_mode_toggles_exact_match_without_changing_input_mode() {
        let mut app = app();
        app.source_items[0].insert("state".into(), "working".into());
        app.source_items[1].insert("state".into(), "idle".into());
        app.filter_config = toml::from_str(
            r#"
            mode = "ctrl-g"

            [[choices]]
            key = "w"
            label = "working"
            source = "state"
            value = "working"
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(app.filter_mode);
        app.handle_key(KeyEvent::from(KeyCode::Char('w')));
        assert_eq!(app.visible, [0]);
        assert_eq!(app.active_filter().unwrap().label, "working");
        app.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.visible, [0, 1]);
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 0);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.filter_mode);
        assert_eq!(app.input_mode, InputMode::Insert);
        assert_eq!(app.outcome, Outcome::Running);

        app.input_mode = InputMode::Normal;
        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::from(KeyCode::Char('w')));
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.visible, [0]);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn fil_002_filtered_selection_preserves_default_action_acceptance() {
        let mut app = app_with_actions();
        app.source_items[0].insert("state".into(), "working".into());
        app.source_items[1].insert("state".into(), "idle".into());
        app.filter_config = toml::from_str(
            r#"
            mode = "ctrl-g"

            [[choices]]
            key = "i"
            label = "idle"
            source = "state"
            value = "idle"
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.visible, [1]);

        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(!app.filter_mode);
        assert_eq!(app.selected_source_item().unwrap()["id"], "2");
        assert_eq!(app.outcome, Outcome::ActionRequested(0));
    }

    #[test]
    fn fil_003_filtered_output_only_selection_accepts_visible_value() {
        let mut app = app();
        app.source_items[0].insert("state".into(), "working".into());
        app.source_items[1].insert("state".into(), "idle".into());
        app.filter_config = toml::from_str(
            r#"
            [[choices]]
            key = "i"
            label = "idle"
            source = "state"
            value = "idle"
            "#,
        )
        .unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert_eq!(app.outcome, Outcome::Accepted("2".into()));
    }

    #[test]
    fn ref_001_keeps_selected_value_across_refresh() {
        let mut app = app();
        app.handle_key(KeyEvent::from(KeyCode::Down));
        let replacement = json!([
            { "id": "2", "name": "Beta updated" },
            { "id": "1", "name": "Alpha" }
        ]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert_eq!(app.selected_item().unwrap().value, "2");
    }

    #[test]
    fn act_007_refresh_preserves_query_and_selected_identity() {
        let mut app = app();
        app.query = "a".into();
        app.cursor = 1;
        app.refilter();
        app.selected = 1;
        let replacement = json!([
            { "id": "2", "name": "Beta updated" },
            { "id": "1", "name": "Alpha" }
        ]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert_eq!(app.query, "a");
        assert_eq!(app.selected_item().unwrap().value, "2");
    }

    #[test]
    fn act_008_refresh_selects_nearby_item_after_deletion() {
        let mut app = app();
        app.source_items.push(
            json!({"id": "3", "name": "Gamma"})
                .as_object()
                .unwrap()
                .clone(),
        );
        app.tick(0);
        app.selected = 1;
        let replacement = json!([
            { "id": "1", "name": "Alpha" },
            { "id": "3", "name": "Gamma" }
        ]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert_eq!(app.selected, 1);
        assert_eq!(app.selected_item().unwrap().value, "3");
    }

    #[test]
    fn act_008_refresh_closes_action_menu_when_selected_item_is_deleted() {
        let mut app = app_with_actions();
        app.selected = 1;
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert!(app.action_menu);
        let replacement = json!([{ "id": "1", "name": "Alpha" }]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert!(!app.action_menu);
        assert_eq!(app.selected_item().unwrap().value, "1");
    }

    #[test]
    fn act_007_refresh_preserves_highlighted_action_identity() {
        let mut app = app_with_actions();
        app.source_items[0].insert("ready".into(), json!(true));
        app.action_config.items[0].when = vec![crate::config::ActionCondition {
            field: "ready".into(),
            equals: Some(json!(true)),
            is_set: None,
        }];
        app.tick(0);
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.action_selected, 1);
        let replacement = json!([
            { "id": "1", "name": "Alpha", "ready": false },
            { "id": "2", "name": "Beta" }
        ]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert!(app.action_menu);
        assert_eq!(app.action_selected, 0);
        assert_eq!(app.matching_action_indices(), [1]);
    }

    #[test]
    fn inp_001_cancel_binding_exits_without_value() {
        let mut app = app();
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.outcome, Outcome::Running);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_002_nav_001_supports_readline_editing_and_list_bindings() {
        let mut app = app();
        for character in "one two".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(app.query, "one ");
        assert_eq!(app.cursor, 4);

        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 0);
        app.query.clear();
        app.cursor = 0;
        app.refilter();
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn inp_003_can_001_supports_basic_vim_input_and_ctrl_c_always_cancels() {
        let mut app = app();
        for character in "abc".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        app.handle_key(KeyEvent::from(KeyCode::Char('h')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(app.query, "ac");
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.input_mode, InputMode::Insert);

        app.keybindings.enabled = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_004_escape_cancels_immediately_when_vim_mode_is_disabled() {
        let mut app = app();
        app.input_config.vim = false;
        app.input_mode = InputMode::Insert;

        app.handle_key(KeyEvent::from(KeyCode::Esc));

        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_005_vim_end_and_delete_keep_cursor_on_the_last_grapheme() {
        let mut app = app();
        for character in "abc".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        app.handle_key(KeyEvent::from(KeyCode::Char('$')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!((app.query.as_str(), app.cursor), ("ab", 1));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!((app.query.as_str(), app.cursor), ("", 0));
    }

    #[test]
    fn inp_006_editing_treats_combining_and_joined_emoji_as_graphemes() {
        for value in ["e\u{301}", "👨‍👩‍👧‍👦"] {
            let mut app = app();
            for character in value.chars() {
                app.handle_key(KeyEvent::from(KeyCode::Char(character)));
            }

            app.handle_key(KeyEvent::from(KeyCode::Backspace));

            assert_eq!(app.query, "");
            assert_eq!(app.cursor, 0);
        }
    }
}
