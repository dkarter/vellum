use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use serde_json::{Map, Value};

use crate::{
    config::{ActionAvailability, ActionConfig},
    item::field_value,
    source::ensure_success,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AvailabilityCommand {
    argv: Vec<String>,
    cwd: Option<String>,
    timeout_ms: u64,
}

pub fn run(action: &ActionConfig, item: &Map<String, Value>) -> Result<()> {
    let label = format!("action '{}'", action.name);
    let mut command = if let Some(argv) = &action.command {
        let argv = interpolate(argv, item)?;
        let mut command = Command::new(&argv[0]);
        command.args(&argv[1..]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            action.shell.as_deref().expect("validated shell action"),
        ]);
        command
    };
    if let Some(cwd) = &action.cwd {
        command.current_dir(interpolate_argument(cwd, item)?);
    }
    command.stdout(Stdio::null());
    let output = crate::source::run_command(&mut command, &label)?;
    ensure_success(&label, &output)
}

pub fn prepare_availability(
    availability: &ActionAvailability,
    item: &Map<String, Value>,
) -> Result<AvailabilityCommand> {
    let argv = interpolate(&availability.command, item)?;
    if argv.is_empty() || argv[0].trim().is_empty() {
        bail!("action availability command cannot be empty");
    }
    Ok(AvailabilityCommand {
        argv,
        cwd: availability
            .cwd
            .as_deref()
            .map(|cwd| interpolate_argument(cwd, item))
            .transpose()?,
        timeout_ms: availability.timeout_ms,
    })
}

pub fn check_availability(check: &AvailabilityCommand) -> bool {
    let mut command = Command::new(&check.argv[0]);
    command
        .args(&check.argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = &check.cwd {
        command.current_dir(cwd);
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let timeout = Duration::from_millis(check.timeout_ms);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(
                    Duration::from_millis(10).min(timeout.saturating_sub(started.elapsed())),
                );
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn interpolate(argv: &[String], item: &Map<String, Value>) -> Result<Vec<String>> {
    argv.iter()
        .map(|argument| interpolate_argument(argument, item))
        .collect()
}

fn interpolate_argument(argument: &str, item: &Map<String, Value>) -> Result<String> {
    let Some(path) = field_expression(argument) else {
        return Ok(argument.to_owned());
    };
    match field_value(item, path) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Null) | None => bail!("action field '${path}' is missing or null"),
        Some(Value::Array(_) | Value::Object(_)) => {
            bail!("action field '${path}' must be a scalar")
        }
    }
}

fn field_expression(argument: &str) -> Option<&str> {
    let path = argument.strip_prefix('$')?;
    let valid = !path.is_empty()
        && path.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
        });
    valid.then_some(path)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::config::{ActionAvailability, Bindings, OnSuccess};

    fn action(command: Vec<&str>) -> ActionConfig {
        ActionConfig {
            name: "check".into(),
            label: "Check".into(),
            icon: String::new(),
            description: String::new(),
            key: Bindings::default(),
            command: Some(command.into_iter().map(str::to_owned).collect()),
            shell: None,
            cwd: None,
            availability: None,
            when: Vec::new(),
            on_success: OnSuccess::Exit,
        }
    }

    #[test]
    fn act_005_argv_interpolation_preserves_argument_boundaries() {
        let item = json!({"value": "spaces; $(exit 9)"})
            .as_object()
            .unwrap()
            .clone();
        let action = action(vec!["test", "$value", "=", "spaces; $(exit 9)"]);

        run(&action, &item).unwrap();
    }

    #[test]
    fn act_006_failed_action_reports_status_and_stderr() {
        let item = Map::new();
        let action = action(vec!["sh", "-c", "printf failure >&2; exit 7"]);

        let error = run(&action, &item).unwrap_err().to_string();

        assert!(error.contains("exit status: 7"), "{error}");
        assert!(error.contains("failure"), "{error}");
    }

    #[test]
    fn act_005_literal_dollar_arguments_are_not_interpolated() {
        let item = Map::new();
        let action = action(vec!["test", "$HOME/path", "=", "$HOME/path"]);

        run(&action, &item).unwrap();
    }

    #[test]
    fn act_010_action_working_directory_interpolates_safely() {
        let root = std::env::temp_dir().join(format!(
            "vellum-action-cwd-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("marker"), "present").unwrap();
        let item = json!({"checkout_path": root}).as_object().unwrap().clone();
        let mut action = action(vec!["test", "-f", "marker"]);
        action.cwd = Some("$checkout_path".into());

        run(&action, &item).unwrap();

        let spawned = root.join("spawned");
        action.command = Some(vec![
            "sh".into(),
            "-c".into(),
            "touch \"$1\"".into(),
            "sh".into(),
            spawned.to_string_lossy().into_owned(),
        ]);
        for item in [
            json!({}),
            json!({"checkout_path": null}),
            json!({"checkout_path": []}),
            json!({"checkout_path": {}}),
        ] {
            assert!(run(&action, item.as_object().unwrap()).is_err());
            assert!(!spawned.exists());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn act_011_availability_uses_interpolated_argv_cwd_and_exit_status() {
        let item = json!({"checkout_path": std::env::temp_dir()})
            .as_object()
            .unwrap()
            .clone();
        let mut availability = ActionAvailability {
            command: vec!["true".into()],
            cwd: Some("$checkout_path".into()),
            cache_ms: 30_000,
            timeout_ms: 5_000,
        };

        assert!(check_availability(
            &prepare_availability(&availability, &item).unwrap()
        ));
        availability.command = vec!["false".into()];
        assert!(!check_availability(
            &prepare_availability(&availability, &item).unwrap()
        ));
        availability.command = vec!["sleep".into(), "1".into()];
        availability.timeout_ms = 10;
        let started = Instant::now();
        assert!(!check_availability(
            &prepare_availability(&availability, &item).unwrap()
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
