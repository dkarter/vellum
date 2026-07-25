use std::{
    env, fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyEventKind};
use vellum::{
    app::{App, Outcome},
    config::Config,
    source, ui,
};

const REFRESH_POLL_RATE: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    let config_path = match cli(env::args().skip(1))? {
        Cli::Run(path) => path,
        Cli::Help => {
            print_help();
            return Ok(());
        }
        Cli::Version => {
            println!("vellum {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    };
    let config_text = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let config = Config::parse(&config_text)?;
    let source_items = source::run(&config.source.cmd)?;
    let mut app = App::new(
        source_items,
        config.item.clone(),
        config.keybindings.clone(),
        config.search.enabled,
    );

    let mut terminal = ratatui::try_init().context("failed to initialize terminal")?;
    let result = run(&mut terminal, &mut app, &config);
    ratatui::restore();
    let outcome = result?;
    if let Outcome::Accepted(value) = outcome {
        println!("{value}");
    }
    Ok(())
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, config: &Config) -> Result<Outcome> {
    let started = Instant::now();
    let mut last_animation = Instant::now();
    let mut last_refresh = Instant::now();
    let mut refresh_result: Option<Receiver<Result<Vec<source::SourceItem>>>> = None;
    let refresh_interval = Duration::from_millis(config.source.refresh_ms);
    let animation_interval = app.animation_interval();
    let mut dirty = true;
    loop {
        if dirty {
            terminal.draw(|frame| ui::render(frame, app, config))?;
            dirty = false;
        }

        let timeout = next_timeout(
            animation_interval,
            last_animation,
            refresh_interval,
            last_refresh,
            refresh_result.is_some(),
        );
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
            dirty = true;
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
        if !refresh_interval.is_zero()
            && refresh_result.is_none()
            && last_refresh.elapsed() >= refresh_interval
        {
            refresh_result = Some(spawn_refresh(config.source.cmd.clone()));
            last_refresh = Instant::now();
        }
    }
}

fn next_timeout(
    animation_interval: Option<Duration>,
    last_animation: Instant,
    refresh_interval: Duration,
    last_refresh: Instant,
    refresh_pending: bool,
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
    timeout
}

fn spawn_refresh(command: String) -> Receiver<Result<Vec<source::SourceItem>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(source::run(&command));
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

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Run(PathBuf),
    Help,
    Version,
}

fn cli(mut args: impl Iterator<Item = String>) -> Result<Cli> {
    let first = args.next();
    if args.next().is_some() {
        bail!("expected at most one configuration path");
    }
    match first.as_deref() {
        Some("-h" | "--help") => Ok(Cli::Help),
        Some("-V" | "--version") => Ok(Cli::Version),
        Some(path) => Ok(Cli::Run(PathBuf::from(path))),
        None => default_config_path().map(Cli::Run),
    }
}

fn default_config_path() -> Result<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home).join("vellum/config.toml"));
    }
    env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config/vellum/config.toml"))
        .context("HOME and XDG_CONFIG_HOME are both unset; pass a configuration path")
}

fn print_help() {
    println!(
        "Vellum {}\n\nUsage: vellum [CONFIG]\n\nArguments:\n  CONFIG  TOML config path [default: $XDG_CONFIG_HOME/vellum/config.toml]\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_path_wins() {
        let command = cli(["custom.toml".into()].into_iter()).unwrap();
        assert_eq!(command, Cli::Run(PathBuf::from("custom.toml")));
    }

    #[test]
    fn help_does_not_load_a_config() {
        assert_eq!(cli(["--help".into()].into_iter()).unwrap(), Cli::Help);
        assert_eq!(cli(["--version".into()].into_iter()).unwrap(), Cli::Version);
    }

    #[test]
    fn rejects_extra_arguments() {
        let error = cli(["one".into(), "two".into()].into_iter()).unwrap_err();
        assert!(error.to_string().contains("at most one"));
    }
}
