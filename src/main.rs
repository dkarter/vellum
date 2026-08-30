use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, Result, bail};
use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use vellum::{
    action,
    app::{App, Outcome},
    config::{Config, OnSuccess},
    frecency::Frecency,
    official, source, ui,
};

const REFRESH_POLL_RATE: Duration = Duration::from_millis(50);
const MAX_EVENTS_PER_TICK: usize = 64;
const STDIN_PALETTE: &str = r#"
[source]
stdin = true

[item]
template = [["$value"]]
value = "$value"
"#;
type Tui = Terminal<CrosstermBackend<io::Stderr>>;

struct TerminalSession {
    terminal: Tui,
    active: bool,
}

fn main() -> Result<()> {
    let run_request = match cli(env::args().skip(1))? {
        Cli::Run(options) => options,
        Cli::PalettesSync { overwrite } => {
            let root = config_root().context(
                "HOME and XDG_CONFIG_HOME are both unset; cannot locate the palette directory",
            )?;
            print_sync_report(&official::sync(&root, overwrite)?);
            return Ok(());
        }
        Cli::Help => {
            print_help();
            return Ok(());
        }
        Cli::Version => {
            println!("vellum {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    };
    let config_root = config_root();
    let palette_path = if run_request.default_stdin_palette {
        PathBuf::from("<stdin>")
    } else {
        palette_path(&run_request.palette, config_root.as_deref())?
    };
    let palette_key = palette_identity(&palette_path);
    let palette = if run_request.default_stdin_palette {
        STDIN_PALETTE.to_owned()
    } else {
        fs::read_to_string(&palette_path)
            .with_context(|| format!("failed to read palette {}", palette_path.display()))?
    };
    let global_path = config_root.map(|root| root.join("config.toml"));
    let global = match &global_path {
        Some(global_path) => match fs::read_to_string(global_path) {
            Ok(global) => Some(global),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", global_path.display()));
            }
        },
        None => None,
    };
    let global = global.as_deref().zip(global_path.as_deref());
    let mut config = Config::parse_layered_files(global, (&palette, &palette_path))?;
    let stdin_source = if run_request.source_cache.is_none() {
        run_request.stdin.clone().or_else(|| {
            config.source.stdin.then(|| source::StdinSource {
                mode: source::StdinMode::Json,
                fields: Vec::new(),
            })
        })
    } else {
        None
    };
    if let Some(stdin) = &stdin_source {
        if config.actions.has_refresh_action() {
            bail!("stdin CLI sources cannot be used with action on_success = 'refresh'");
        }
        let items = source::run_stdin(stdin)?;
        return rerun_with_terminal_input(
            &run_request.palette,
            run_request.default_stdin_palette,
            &items,
        );
    }
    let source_items = if let Some(cache) = &run_request.source_cache {
        if config.actions.has_refresh_action() {
            bail!("stdin CLI sources cannot be used with action on_success = 'refresh'");
        }
        config.source.refresh_ms = 0;
        source::load_json(cache)?
    } else {
        source::run(&config.source)?
    };
    let mut frecency = if config.frecency.enabled {
        let root = data_root().context(
            "HOME, XDG_DATA_HOME, and VELLUM_DATA are unset; cannot store frecency data",
        )?;
        match Frecency::load(&root, config.frecency.max_entries) {
            Ok(frecency) => Some(frecency),
            Err(error) => {
                eprintln!("warning: frecency disabled: {error:#}");
                None
            }
        }
    } else {
        None
    };
    let frecency_scores = match frecency
        .as_ref()
        .map(|frecency| frecency.scores(&palette_key))
        .transpose()
    {
        Ok(scores) => scores.unwrap_or_default(),
        Err(error) => {
            eprintln!("warning: frecency disabled: {error:#}");
            frecency = None;
            Default::default()
        }
    };
    let mut app = App::new_with_frecency_and_actions(
        source_items,
        config.item.clone(),
        config.keybindings.clone(),
        config.filters.clone(),
        config.input.clone(),
        config.search.enabled,
        frecency_scores,
        config.actions.clone(),
    );

    let mut terminal = TerminalSession::init().context("failed to initialize terminal")?;
    let result = run(&mut terminal.terminal, &mut app, &config);
    terminal.restore()?;
    let outcome = result?;
    if let Outcome::Accepted(value) = &outcome
        && let Err(error) = record_selection(frecency.as_mut(), &palette_key, value)
    {
        eprintln!("warning: failed to record frecency: {error:#}");
    }
    write_outcome(&mut io::stdout(), &outcome)?;
    Ok(())
}

fn record_selection(frecency: Option<&mut Frecency>, palette: &str, value: &str) -> Result<()> {
    if let Some(frecency) = frecency {
        frecency.record(palette, value)?;
    }
    Ok(())
}

impl TerminalSession {
    fn init() -> Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        if let Err(error) = execute!(stderr, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        match Terminal::new(CrosstermBackend::new(stderr)) {
            Ok(terminal) => Ok(Self {
                terminal,
                active: true,
            }),
            Err(error) => {
                let mut stderr = io::stderr();
                let _ = execute!(stderr, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                Err(error.into())
            }
        }
    }

    fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let terminal_result = execute!(
            self.terminal.backend_mut(),
            SetCursorStyle::DefaultUserShape,
            Show,
            LeaveAlternateScreen
        );
        let raw_mode_result = disable_raw_mode();
        if terminal_result.is_ok() && raw_mode_result.is_ok() {
            self.active = false;
        }
        terminal_result?;
        raw_mode_result?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn write_outcome(writer: &mut impl Write, outcome: &Outcome) -> Result<()> {
    if let Outcome::Accepted(value) = outcome {
        writeln!(writer, "{value}")?;
    }
    Ok(())
}

fn run(terminal: &mut Tui, app: &mut App, config: &Config) -> Result<Outcome> {
    let started = Instant::now();
    let mut last_animation = Instant::now();
    let mut last_refresh = Instant::now();
    let mut refresh_result: Option<Receiver<Result<Vec<source::SourceItem>>>> = None;
    let mut availability_results: Vec<Receiver<(action::AvailabilityCommand, bool)>> = Vec::new();
    let refresh_interval = Duration::from_millis(config.source.refresh_ms);
    let animation_interval = app.animation_interval();
    let mut dirty = true;
    let mut cursor_mode = None;
    loop {
        if dirty {
            terminal.draw(|frame| ui::render(frame, app, config))?;
            let desired_cursor = if app.action_menu {
                vellum::config::InputMode::Insert
            } else {
                app.input_mode
            };
            if cursor_mode != Some(desired_cursor) {
                set_cursor_style(terminal, desired_cursor)?;
                cursor_mode = Some(desired_cursor);
            }
            dirty = false;
        }

        let timeout = if app.outcome == Outcome::Running {
            next_timeout(
                animation_interval,
                last_animation,
                refresh_interval,
                last_refresh,
                refresh_result.is_some() || !availability_results.is_empty(),
                app.availability_refresh_in(),
            )
        } else {
            Duration::ZERO
        };
        if event::poll(timeout)? {
            for _ in 0..MAX_EVENTS_PER_TICK {
                dirty |= handle_terminal_event(app, event::read()?);
                if app.outcome != Outcome::Running || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        if let Outcome::ActionRequested(index) = app.outcome {
            dirty = true;
            refresh_result = None;
            availability_results.clear();
            app.invalidate_availability();
            let action = &config.actions.items[index];
            let item = app
                .selected_source_item()
                .context("selected action has no source item")?;
            match action::run(action, item) {
                Ok(()) if action.on_success == OnSuccess::Exit => {
                    return Ok(Outcome::ActionCompleted);
                }
                Ok(()) => match source::run(&config.source) {
                    Ok(items) => {
                        app.replace_source(items, started.elapsed().as_millis() as u64);
                        app.finish_action(None);
                        last_refresh = Instant::now();
                    }
                    Err(error) => app.finish_action(Some(format!("refresh failed: {error:#}"))),
                },
                Err(error) => app.finish_action(Some(format!("action failed: {error:#}"))),
            }
        }

        if app.outcome != Outcome::Running {
            return Ok(app.outcome.clone());
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if animation_interval.is_some_and(|interval| last_animation.elapsed() >= interval) {
            app.tick(elapsed_ms);
            last_animation = Instant::now();
            dirty = true;
        }

        if let Some(result) = receive_refresh(&refresh_result)? {
            dirty |= app.replace_source(result?, elapsed_ms);
            refresh_result = None;
        }
        let completed_availability = receive_availability(&mut availability_results)?;
        if !completed_availability.is_empty() {
            for (check, available) in completed_availability {
                app.finish_availability_check(check, available);
            }
            dirty = true;
        }
        availability_results.extend(
            app.take_availability_checks()
                .into_iter()
                .map(spawn_availability_check),
        );
        if !refresh_interval.is_zero()
            && refresh_result.is_none()
            && last_refresh.elapsed() >= refresh_interval
        {
            refresh_result = Some(spawn_refresh(config.source.clone()));
            last_refresh = Instant::now();
        }
    }
}

fn handle_terminal_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            app.handle_key(key);
            true
        }
        Event::Resize(_, _) => true,
        _ => false,
    }
}

fn set_cursor_style(terminal: &mut Tui, mode: vellum::config::InputMode) -> Result<()> {
    execute!(terminal.backend_mut(), cursor_style(mode))?;
    Ok(())
}

fn cursor_style(mode: vellum::config::InputMode) -> SetCursorStyle {
    match mode {
        vellum::config::InputMode::Insert => SetCursorStyle::SteadyBar,
        vellum::config::InputMode::Normal => SetCursorStyle::SteadyBlock,
    }
}

fn next_timeout(
    animation_interval: Option<Duration>,
    last_animation: Instant,
    refresh_interval: Duration,
    last_refresh: Instant,
    refresh_pending: bool,
    availability_refresh_in: Option<Duration>,
) -> Duration {
    let mut timeout = Duration::from_secs(3_600);
    if let Some(interval) = animation_interval {
        timeout = timeout.min(interval.saturating_sub(last_animation.elapsed()));
    }
    if !refresh_interval.is_zero() && !refresh_pending {
        timeout = timeout.min(refresh_interval.saturating_sub(last_refresh.elapsed()));
    }
    if refresh_pending {
        timeout = timeout.min(REFRESH_POLL_RATE);
    }
    if let Some(refresh_in) = availability_refresh_in {
        timeout = timeout.min(refresh_in);
    }
    timeout
}

fn spawn_refresh(
    source: vellum::config::SourceConfig,
) -> Receiver<Result<Vec<source::SourceItem>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(source::run(&source));
    });
    receiver
}

fn receive_refresh(
    receiver: &Option<Receiver<Result<Vec<source::SourceItem>>>>,
) -> Result<Option<Result<Vec<source::SourceItem>>>> {
    match receiver.as_ref().map(Receiver::try_recv) {
        Some(Ok(result)) => Ok(Some(result)),
        Some(Err(TryRecvError::Disconnected)) => bail!("source refresh worker disconnected"),
        Some(Err(TryRecvError::Empty)) | None => Ok(None),
    }
}

fn spawn_availability_check(
    check: action::AvailabilityCommand,
) -> Receiver<(action::AvailabilityCommand, bool)> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let available = action::check_availability(&check);
        let _ = sender.send((check, available));
    });
    receiver
}

fn receive_availability(
    receivers: &mut Vec<Receiver<(action::AvailabilityCommand, bool)>>,
) -> Result<Vec<(action::AvailabilityCommand, bool)>> {
    let mut completed = Vec::new();
    let mut index = 0;
    while index < receivers.len() {
        match receivers[index].try_recv() {
            Ok(result) => {
                completed.push(result);
                receivers.swap_remove(index);
            }
            Err(TryRecvError::Disconnected) => {
                bail!("action availability worker disconnected")
            }
            Err(TryRecvError::Empty) => index += 1,
        }
    }
    Ok(completed)
}

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Run(RunOptions),
    PalettesSync { overwrite: bool },
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
struct RunOptions {
    palette: String,
    stdin: Option<source::StdinSource>,
    source_cache: Option<PathBuf>,
    default_stdin_palette: bool,
}

fn cli(args: impl Iterator<Item = String>) -> Result<Cli> {
    let args: Vec<_> = args.collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["-h" | "--help"] => Ok(Cli::Help),
        ["-V" | "--version"] => Ok(Cli::Version),
        ["palettes", "sync"] => Ok(Cli::PalettesSync { overwrite: false }),
        ["palettes", "sync", "--overwrite"] => Ok(Cli::PalettesSync { overwrite: true }),
        _ => parse_run_options(&args).map(Cli::Run),
    }
}

fn parse_run_options(args: &[String]) -> Result<RunOptions> {
    let mut palette = None;
    let mut mode = None;
    let mut fields = Vec::new();
    let mut source_cache = None;
    let mut default_stdin_palette = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--stdin" => set_stdin_mode(&mut mode, source::StdinMode::Auto)?,
            "--lines" => {
                index += 1;
                let field = required_flag_value(args, index, "--lines")?;
                set_stdin_mode(
                    &mut mode,
                    source::StdinMode::Lines {
                        field: field.to_owned(),
                    },
                )?;
            }
            "--jq" => {
                index += 1;
                let filter = required_flag_value(args, index, "--jq")?;
                set_stdin_mode(
                    &mut mode,
                    source::StdinMode::Jq {
                        filter: filter.to_owned(),
                    },
                )?;
            }
            "--field" => {
                index += 1;
                let mapping = required_flag_value(args, index, "--field")?;
                let (target, source_path) = mapping.split_once('=').with_context(|| {
                    format!("invalid --field '{mapping}'; expected TARGET=SOURCE")
                })?;
                if target.is_empty() || source_path.is_empty() {
                    bail!("invalid --field '{mapping}'; expected nonempty TARGET=SOURCE");
                }
                fields.push(source::FieldMapping {
                    target: target.to_owned(),
                    source: source_path.to_owned(),
                });
            }
            "--stdin-cache" => {
                index += 1;
                let path = required_flag_value(args, index, "--stdin-cache")?;
                if source_cache.replace(PathBuf::from(path)).is_some() {
                    bail!("--stdin-cache may only be set once");
                }
            }
            "--stdin-default-palette" => default_stdin_palette = true,
            argument if argument.starts_with('-') => {
                bail!("unknown option '{argument}'; run 'vellum --help' for usage")
            }
            argument if palette.is_none() => palette = Some(argument.to_owned()),
            _ => bail!("invalid arguments; run 'vellum --help' for usage"),
        }
        index += 1;
    }
    if !fields.is_empty() && mode.is_none() {
        bail!("--field requires --stdin, --lines, or --jq");
    }
    if mode.is_some() && source_cache.is_some() {
        bail!("stdin source options conflict with internal source cache");
    }
    let default_stdin_palette = default_stdin_palette
        || (palette.is_none() && matches!(mode, Some(source::StdinMode::Auto)));
    Ok(RunOptions {
        palette: palette.unwrap_or_else(|| "default".into()),
        stdin: mode.map(|mode| source::StdinSource { mode, fields }),
        source_cache,
        default_stdin_palette,
    })
}

fn required_flag_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str> {
    args.get(index)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .map(String::as_str)
        .with_context(|| format!("{flag} requires a value"))
}

struct SourceCache(PathBuf);

impl Drop for SourceCache {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn rerun_with_terminal_input(
    palette: &str,
    default_stdin_palette: bool,
    items: &[source::SourceItem],
) -> Result<()> {
    let cache = write_source_cache(items)?;
    let terminal_input = open_terminal_input()?;
    let mut command =
        Command::new(env::current_exe().context("failed to locate Vellum executable")?);
    if !default_stdin_palette {
        command.arg(palette);
    }
    command.arg("--stdin-cache").arg(&cache.0);
    if default_stdin_palette {
        command.arg("--stdin-default-palette");
    }
    let status = command
        .stdin(Stdio::from(terminal_input))
        .status()
        .context("failed to restart Vellum with terminal input")?;
    drop(cache);
    if status.success() {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(unix)]
fn open_terminal_input() -> Result<fs::File> {
    let terminal = io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .context("failed to duplicate the terminal for interactive input")?;
    Ok(fs::File::from(terminal))
}

#[cfg(not(unix))]
fn open_terminal_input() -> Result<fs::File> {
    bail!("stdin sources require a Unix terminal")
}

fn write_source_cache(items: &[source::SourceItem]) -> Result<SourceCache> {
    for attempt in 0..100 {
        let path = env::temp_dir().join(format!(
            "vellum-stdin-{}-{attempt}.json",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path);
        match file {
            Ok(mut file) => {
                if let Err(error) = serde_json::to_writer(&mut file, items) {
                    let _ = fs::remove_file(&path);
                    return Err(error).context("failed to write stdin source cache");
                }
                if let Err(error) = file.flush() {
                    let _ = fs::remove_file(&path);
                    return Err(error).context("failed to flush stdin source cache");
                }
                return Ok(SourceCache(path));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("failed to create stdin source cache"),
        }
    }
    bail!("failed to create a unique stdin source cache")
}

fn set_stdin_mode(target: &mut Option<source::StdinMode>, mode: source::StdinMode) -> Result<()> {
    if target.is_some() {
        bail!("use only one of --stdin, --lines, or --jq");
    }
    *target = Some(mode);
    Ok(())
}

fn config_root() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("vellum"));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/vellum"))
}

fn data_root() -> Option<PathBuf> {
    data_root_from(
        env::var_os("VELLUM_DATA").map(PathBuf::from),
        env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        env::var_os("HOME").map(PathBuf::from),
    )
}

fn data_root_from(
    vellum_data: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    vellum_data
        .filter(|root| root.is_absolute() && !root.as_os_str().is_empty())
        .or_else(|| {
            xdg_data_home
                .filter(|root| root.is_absolute() && !root.as_os_str().is_empty())
                .map(|root| root.join("vellum"))
        })
        .or_else(|| {
            home.filter(|root| root.is_absolute() && !root.as_os_str().is_empty())
                .map(|root| root.join(".local/share/vellum"))
        })
}

fn palette_path(palette: &str, config_root: Option<&std::path::Path>) -> Result<PathBuf> {
    let path = std::path::Path::new(palette);
    if path.components().count() > 1 || path.extension().is_some() {
        Ok(path.to_owned())
    } else {
        Ok(config_root
            .context("HOME and XDG_CONFIG_HOME are both unset; use an explicit palette path")?
            .join("palettes")
            .join(palette)
            .with_extension("toml"))
    }
}

fn palette_identity(path: &std::path::Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_owned())
        .to_string_lossy()
        .into_owned()
}

fn print_help() {
    println!(
        "Vellum {}\n\nUsage:\n  vellum [PALETTE] [SOURCE OPTIONS]\n  vellum palettes sync [--overwrite]\n\nArguments:\n  PALETTE  Palette name or TOML path [default: default]\n\nCommands:\n  palettes sync  Install bundled palettes without replacing existing files\n\nSource options:\n  --stdin                 Auto-detect plain lines, JSON, or NDJSON from standard input\n  --lines FIELD           Wrap each nonempty input line as {{FIELD: line}}\n  --field TARGET=SOURCE   Copy a dotted source field to a target field (repeatable)\n  --jq FILTER             Transform standard-input JSON through jq\n\nOptions:\n  --overwrite    Replace existing official palette files during sync\n  -h, --help     Print help\n  -V, --version  Print version",
        env!("CARGO_PKG_VERSION")
    );
}

fn print_sync_report(report: &official::SyncReport) {
    for filename in &report.installed {
        println!("installed {filename}");
    }
    for filename in &report.overwritten {
        println!("overwrote {filename}");
    }
    for filename in &report.skipped {
        println!("skipped {filename} (already exists)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn act_011_availability_checks_run_on_a_background_worker() {
        let availability = vellum::config::ActionAvailability {
            command: vec!["true".into()],
            cwd: None,
            cache_ms: 30_000,
            timeout_ms: 5_000,
        };
        let check = action::prepare_availability(&availability, &Default::default()).unwrap();

        let result = spawn_availability_check(check)
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        assert!(result.1);
    }

    #[test]
    fn cli_001_explicit_config_path_wins() {
        let command = cli(["custom.toml".into()].into_iter()).unwrap();
        assert_eq!(
            command,
            Cli::Run(RunOptions {
                palette: "custom.toml".into(),
                stdin: None,
                source_cache: None,
                default_stdin_palette: false,
            })
        );
    }

    #[test]
    fn cli_002_help_does_not_load_a_config() {
        assert_eq!(cli(["--help".into()].into_iter()).unwrap(), Cli::Help);
        assert_eq!(cli(["--version".into()].into_iter()).unwrap(), Cli::Version);
    }

    #[test]
    fn cli_003_rejects_extra_arguments() {
        let error = cli(["one".into(), "two".into()].into_iter()).unwrap_err();
        assert!(error.to_string().contains("invalid arguments"));
    }

    #[test]
    fn cli_004_resolves_names_under_palette_directory_and_preserves_paths() {
        let root = PathBuf::from("/tmp/vellum");
        assert_eq!(
            palette_path("agents", Some(&root)).unwrap(),
            root.join("palettes/agents.toml")
        );
        assert_eq!(
            palette_path("examples/demo.toml", None).unwrap(),
            PathBuf::from("examples/demo.toml")
        );
    }

    #[test]
    fn cli_005_defaults_to_the_default_named_palette() {
        assert_eq!(
            cli(std::iter::empty()).unwrap(),
            Cli::Run(RunOptions {
                palette: "default".into(),
                stdin: None,
                source_cache: None,
                default_stdin_palette: false,
            })
        );
    }

    #[test]
    fn ui_006_uses_mode_specific_cursor_shapes() {
        assert_eq!(
            cursor_style(vellum::config::InputMode::Insert),
            SetCursorStyle::SteadyBar
        );
        assert_eq!(
            cursor_style(vellum::config::InputMode::Normal),
            SetCursorStyle::SteadyBlock
        );
    }

    #[test]
    fn ui_008_resize_event_requests_a_redraw() {
        let config = Config::parse(
            "[source]\ncmd = 'unused'\n[item]\ntemplate = [['$name']]\nvalue = '$id'",
        )
        .unwrap();
        let mut app = App::new(
            Vec::new(),
            config.item,
            config.keybindings,
            config.filters,
            config.input,
            true,
        );

        assert!(handle_terminal_event(&mut app, Event::Resize(120, 40)));
        assert!(!handle_terminal_event(&mut app, Event::FocusGained));
    }

    #[test]
    fn cli_006_parses_palette_sync_commands() {
        assert_eq!(
            cli(["palettes".into(), "sync".into()].into_iter()).unwrap(),
            Cli::PalettesSync { overwrite: false }
        );
        assert_eq!(
            cli(["palettes".into(), "sync".into(), "--overwrite".into()].into_iter()).unwrap(),
            Cli::PalettesSync { overwrite: true }
        );
    }

    #[test]
    fn cli_007_parses_standard_input_source_flags() {
        let command = cli([
            "agents".into(),
            "--stdin".into(),
            "--field".into(),
            "title=details.name".into(),
            "--field".into(),
            "value=id".into(),
        ]
        .into_iter())
        .unwrap();
        assert_eq!(
            command,
            Cli::Run(RunOptions {
                palette: "agents".into(),
                stdin: Some(source::StdinSource {
                    mode: source::StdinMode::Auto,
                    fields: vec![
                        source::FieldMapping {
                            target: "title".into(),
                            source: "details.name".into(),
                        },
                        source::FieldMapping {
                            target: "value".into(),
                            source: "id".into(),
                        },
                    ],
                }),
                source_cache: None,
                default_stdin_palette: false,
            })
        );

        assert!(cli(["--lines".into(), "path".into()].into_iter()).is_ok());
        assert!(cli(["--jq".into(), ".[]".into()].into_iter()).is_ok());
        assert!(cli(["--stdin".into(), "--jq".into(), ".".into()].into_iter()).is_err());
        assert!(cli(["--field".into(), "name=id".into()].into_iter()).is_err());
        assert!(cli(["--stdin".into(), "--field".into(), "broken".into()].into_iter()).is_err());
        assert!(cli(["--lines".into(), "--stdin".into()].into_iter()).is_err());
        assert!(cli(["--jq".into(), "--stdin".into()].into_iter()).is_err());
    }

    #[test]
    fn cli_008_stdin_without_a_palette_uses_a_generic_finder() {
        let command = cli(["--stdin".into()].into_iter()).unwrap();

        let Cli::Run(options) = command else {
            panic!("expected run command");
        };
        assert_eq!(options.palette, "default");
        assert!(options.default_stdin_palette);
        assert_eq!(options.stdin.unwrap().mode, source::StdinMode::Auto);
    }

    #[test]
    fn frc_006_data_root_honors_environment_precedence() {
        assert_eq!(
            data_root_from(
                Some("/custom/vellum".into()),
                Some("/xdg".into()),
                Some("/home/user".into())
            ),
            Some("/custom/vellum".into())
        );
        assert_eq!(
            data_root_from(None, Some("/xdg".into()), Some("/home/user".into())),
            Some("/xdg/vellum".into())
        );
        assert_eq!(
            data_root_from(None, None, Some("/home/user".into())),
            Some("/home/user/.local/share/vellum".into())
        );
        assert_eq!(
            data_root_from(None, Some("relative".into()), Some("/home/user".into())),
            Some("/home/user/.local/share/vellum".into())
        );
    }

    #[test]
    fn frc_007_accepted_value_is_recorded_before_output() {
        let root =
            std::env::temp_dir().join(format!("vellum-main-frecency-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut frecency = Frecency::load(&root, 100).unwrap();

        record_selection(Some(&mut frecency), "agents", "w1:p1").unwrap();

        let loaded = Frecency::load(&root, 100).unwrap();
        assert!(loaded.scores("agents").unwrap().contains_key("w1:p1"));
        drop(loaded);
        drop(frecency);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frc_009_palette_identity_uses_resolved_canonical_path() {
        let current = std::env::current_dir().unwrap();

        assert_eq!(
            palette_identity(std::path::Path::new(".")),
            current.to_string_lossy()
        );
        assert_eq!(palette_identity(&current), current.to_string_lossy());
    }

    #[test]
    fn out_001_accepted_output_contains_only_the_selected_value() {
        let mut stdout = Vec::new();

        write_outcome(&mut stdout, &Outcome::Accepted("workspace-42".into())).unwrap();

        assert_eq!(stdout, b"workspace-42\n");
    }

    #[test]
    fn out_002_cancellation_has_no_output() {
        let mut stdout = Vec::new();

        write_outcome(&mut stdout, &Outcome::Cancelled).unwrap();

        assert!(stdout.is_empty());
    }
}
