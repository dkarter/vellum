use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

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
type Tui = Terminal<CrosstermBackend<io::Stderr>>;

struct TerminalSession {
    terminal: Tui,
    active: bool,
}

fn main() -> Result<()> {
    let palette_request = match cli(env::args().skip(1))? {
        Cli::Run(palette) => palette,
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
    let palette_path = palette_path(&palette_request, config_root.as_deref())?;
    let palette_key = palette_identity(&palette_path);
    let palette = fs::read_to_string(&palette_path)
        .with_context(|| format!("failed to read palette {}", palette_path.display()))?;
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
    let config = Config::parse_layered_files(global, (&palette, &palette_path))?;
    let source_items = source::run(&config.source)?;
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
    Run(String),
    PalettesSync { overwrite: bool },
    Help,
    Version,
}

fn cli(args: impl Iterator<Item = String>) -> Result<Cli> {
    let args: Vec<_> = args.collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => Ok(Cli::Run("default".into())),
        ["-h" | "--help"] => Ok(Cli::Help),
        ["-V" | "--version"] => Ok(Cli::Version),
        ["palettes", "sync"] => Ok(Cli::PalettesSync { overwrite: false }),
        ["palettes", "sync", "--overwrite"] => Ok(Cli::PalettesSync { overwrite: true }),
        [palette] => Ok(Cli::Run((*palette).to_owned())),
        _ => bail!("invalid arguments; run 'vellum --help' for usage"),
    }
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
        "Vellum {}\n\nUsage:\n  vellum [PALETTE]\n  vellum palettes sync [--overwrite]\n\nArguments:\n  PALETTE  Palette name or TOML path [default: default]\n\nCommands:\n  palettes sync  Install bundled palettes without replacing existing files\n\nOptions:\n  --overwrite    Replace existing official palette files during sync\n  -h, --help     Print help\n  -V, --version  Print version",
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
        assert_eq!(command, Cli::Run("custom.toml".into()));
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
        assert_eq!(cli(std::iter::empty()).unwrap(), Cli::Run("default".into()));
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
