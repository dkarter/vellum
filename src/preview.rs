use std::{
    collections::VecDeque,
    io::Read,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use serde_json::{Map, Value};
#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::{action, app::App, config::PreviewConfig};

const MAX_OUTPUT: u64 = 128 * 1024;

pub struct Worker {
    receiver: Receiver<Arc<Text<'static>>>,
    cancelled: Arc<AtomicBool>,
}

impl Worker {
    pub fn start(config: PreviewConfig, item: Map<String, Value>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        thread::spawn(move || {
            let output = run(&config, &item, &flag);
            let _ = sender.send(Arc::new(parse_ansi(&output)));
        });
        Self {
            receiver,
            cancelled,
        }
    }

    pub fn try_recv(&self) -> Option<Arc<Text<'static>>> {
        self.receiver.try_recv().ok()
    }
}

/// Most recent 50 previews, capped at 128 KiB per command by the worker.
#[derive(Default)]
pub struct Cache {
    entries: VecDeque<(Map<String, Value>, Arc<Text<'static>>)>,
}

#[derive(Default)]
pub struct Controller {
    selected: Option<Map<String, Value>>,
    worker: Option<Worker>,
    cache: Cache,
}

impl Controller {
    /// Keep the last frame intact until the next result arrives, like Television.
    pub fn update(&mut self, app: &mut App, config: &PreviewConfig) -> bool {
        if !config.enabled {
            return false;
        }
        let selected = app.selected_source_item().cloned();
        if selected != self.selected {
            self.worker = None;
            self.selected = selected;
            if let Some(item) = &self.selected {
                if let Some(content) = self.cache.get(item) {
                    app.set_preview_content(content);
                    return true;
                }
                self.worker = Some(Worker::start(config.clone(), item.clone()));
            } else {
                app.set_preview_text(String::new());
                return true;
            }
        }
        if let Some(content) = self.worker.as_ref().and_then(Worker::try_recv) {
            self.worker = None;
            if let Some(item) = &self.selected {
                self.cache.insert(item.clone(), Arc::clone(&content));
            }
            app.set_preview_content(content);
            return true;
        }
        false
    }

    pub fn pending(&self) -> bool {
        self.worker.is_some()
    }
}

impl Cache {
    pub fn get(&mut self, item: &Map<String, Value>) -> Option<Arc<Text<'static>>> {
        let index = self.entries.iter().position(|(key, _)| key == item)?;
        let entry = self.entries.remove(index)?;
        let content = Arc::clone(&entry.1);
        self.entries.push_back(entry);
        Some(content)
    }

    pub fn insert(&mut self, item: Map<String, Value>, content: Arc<Text<'static>>) {
        if let Some(index) = self.entries.iter().position(|(key, _)| key == &item) {
            self.entries.remove(index);
        }
        if self.entries.len() == 50 {
            self.entries.pop_front();
        }
        self.entries.push_back((item, content));
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn run(config: &PreviewConfig, item: &Map<String, Value>, cancelled: &AtomicBool) -> String {
    let result = (|| -> anyhow::Result<String> {
        let argv = action::interpolate(config.command.as_deref().unwrap_or_default(), item)?;
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(cwd) = &config.cwd {
            command.current_dir(action::interpolate_argument(cwd, item)?);
        }
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(MAX_OUTPUT).read_to_end(&mut bytes);
            let _ = tx.send((result, bytes));
        });
        let started = Instant::now();
        let mut output = None;
        loop {
            if cancelled.load(Ordering::Relaxed)
                || started.elapsed() >= Duration::from_millis(config.timeout_ms)
            {
                stop(&mut child);
                let _ = child.wait();
                anyhow::bail!("preview timed out or was cancelled");
            }
            if output.is_none()
                && let Ok((read, bytes)) = rx.try_recv()
            {
                if bytes.len() as u64 == MAX_OUTPUT {
                    stop(&mut child);
                }
                output = Some((read, bytes));
            }
            if let Some(status) = child.try_wait()?
                && let Some((read, bytes)) = output.take()
            {
                read?;
                if !status.success() && bytes.len() as u64 != MAX_OUTPUT {
                    anyhow::bail!("preview command exited with {status}");
                }
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    result.unwrap_or_else(|error| format!("Preview unavailable: {error}"))
}

fn stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(-pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.kill();
}

/// Interpret only SGR color/style codes. All other terminal controls are discarded.
pub fn parse_ansi(input: &str) -> Text<'static> {
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut pending = String::new();
    let mut style = Style::default();
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    let mut parameters = String::new();
                    let mut final_byte = None;
                    for ch in chars.by_ref() {
                        if ('@'..='~').contains(&ch) {
                            final_byte = Some(ch);
                            break;
                        }
                        if parameters.len() < 64 {
                            parameters.push(ch);
                        }
                    }
                    if final_byte == Some('m') {
                        flush_span(&mut spans, &mut pending, style);
                        sgr(&parameters, &mut style);
                    }
                }
                Some(']') => {
                    for ch in chars.by_ref() {
                        if ch == '\u{7}' {
                            break;
                        }
                        if ch == '\u{1b}' {
                            let _ = chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if character == '\n' {
            flush_span(&mut spans, &mut pending, style);
            lines.push(Line::from(std::mem::take(&mut spans)));
        } else if character == '\t' {
            pending.push_str("    ");
        } else if !character.is_control() {
            pending.push(character);
        }
    }
    flush_span(&mut spans, &mut pending, style);
    lines.push(Line::from(spans));
    Text::from(lines)
}

fn flush_span(spans: &mut Vec<Span<'static>>, pending: &mut String, style: Style) {
    if !pending.is_empty() {
        spans.push(Span::styled(std::mem::take(pending), style));
    }
}

fn sgr(parameters: &str, style: &mut Style) {
    let codes: Vec<u16> = parameters
        .split(';')
        .map(|code| code.parse().unwrap_or(0))
        .collect();
    let mut i = 0;
    while i < codes.len() {
        match codes[i] {
            0 => *style = Style::default(),
            1 => *style = style.add_modifier(Modifier::BOLD),
            3 => *style = style.add_modifier(Modifier::ITALIC),
            4 => *style = style.add_modifier(Modifier::UNDERLINED),
            22 => *style = style.remove_modifier(Modifier::BOLD),
            23 => *style = style.remove_modifier(Modifier::ITALIC),
            24 => *style = style.remove_modifier(Modifier::UNDERLINED),
            30..=37 | 90..=97 => *style = style.fg(ansi_color(codes[i])),
            40..=47 | 100..=107 => *style = style.bg(ansi_color(codes[i] - 10)),
            39 => style.fg = None,
            49 => style.bg = None,
            38 | 48 if i + 2 < codes.len() => {
                let foreground = codes[i] == 38;
                let (color, consumed) = match codes[i + 1] {
                    5 if codes[i + 2] <= 255 => (Some(Color::Indexed(codes[i + 2] as u8)), 2),
                    2 if i + 4 < codes.len()
                        && codes[i + 2..=i + 4].iter().all(|value| *value <= 255) =>
                    {
                        (
                            Some(Color::Rgb(
                                codes[i + 2] as u8,
                                codes[i + 3] as u8,
                                codes[i + 4] as u8,
                            )),
                            4,
                        )
                    }
                    _ => (None, 0),
                };
                if let Some(color) = color {
                    *style = if foreground {
                        style.fg(color)
                    } else {
                        style.bg(color)
                    };
                    i += consumed;
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn ansi_color(code: u16) -> Color {
    Color::Indexed(if code >= 90 {
        (code - 90 + 8) as u8
    } else {
        (code - 30) as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Config};

    #[test]
    fn ui_019_recent_preview_is_instant_and_pending_selection_retains_content() {
        let config = Config::parse("[source]\ncmd='unused'\n[item]\ntemplate=[['$id']]\nvalue='$id'\n[preview]\nenabled=true\ncommand=['printf','%s\\n','$id']").unwrap();
        let items = ["one", "two"]
            .into_iter()
            .map(|id| serde_json::json!({"id":id}).as_object().unwrap().clone())
            .collect();
        let mut app = App::new(
            items,
            config.item,
            config.keybindings,
            config.filters,
            config.input,
            true,
        );
        let mut controller = Controller::default();
        assert!(!controller.update(&mut app, &config.preview));
        assert!(controller.pending());
        for _ in 0..100 {
            if controller.update(&mut app, &config.preview) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.preview_lines.lines[0].spans[0].content, "one");
        app.selected = 1;
        assert!(!controller.update(&mut app, &config.preview));
        assert_eq!(app.preview_lines.lines[0].spans[0].content, "one");
        app.selected = 0;
        assert!(controller.update(&mut app, &config.preview));
        assert!(!controller.pending());
        assert_eq!(app.preview_lines.lines[0].spans[0].content, "one");
        for id in 0..52 {
            let item = serde_json::json!({"id":id}).as_object().unwrap().clone();
            controller
                .cache
                .insert(item, Arc::new(Text::raw(format!("{id}"))));
        }
        assert_eq!(controller.cache.entries.len(), 50);
    }

    #[test]
    fn ui_015_preview_interpolates_and_discards_control_sequences() {
        let config = PreviewConfig {
            command: Some(vec!["printf".into(), "%s\\n".into(), "$path".into()]),
            ..PreviewConfig::default()
        };
        let item = serde_json::json!({"path": "a path; $(echo unsafe)"});
        let worker = Worker::start(config, item.as_object().unwrap().clone());
        let mut result = None;
        for _ in 0..100 {
            result = worker.try_recv();
            if result.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            result.unwrap().lines[0].spans[0].content,
            "a path; $(echo unsafe)"
        );
        let text = parse_ansi("\u{1b}[31mred\u{1b}[0m\u{1b}]0;title\u{7} text");
        assert_eq!(text.lines[0].spans[0].content, "red");
        assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Indexed(1)));
        assert_eq!(text.lines[0].spans[1].content, " text");
        let rgb = parse_ansi("\u{1b}[38;2;10;20;30mcolor\u{1b}[48;5;42m!");
        assert_eq!(rgb.lines[0].spans[0].style.fg, Some(Color::Rgb(10, 20, 30)));
        assert_eq!(rgb.lines[0].spans[1].style.bg, Some(Color::Indexed(42)));
    }

    #[test]
    fn ui_015_slow_preview_times_out_without_blocking_the_caller() {
        let config = PreviewConfig {
            command: Some(vec!["sleep".into(), "5".into()]),
            timeout_ms: 40,
            ..PreviewConfig::default()
        };
        let started = Instant::now();
        let worker = Worker::start(config, Map::new());
        assert!(started.elapsed() < Duration::from_millis(30));
        let mut output = None;
        for _ in 0..100 {
            output = worker.try_recv();
            if output.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            output.unwrap().lines[0].spans[0]
                .content
                .contains("timed out")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
