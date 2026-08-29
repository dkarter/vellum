use std::process::{Command, Stdio};

use anyhow::{Result, bail};
use serde_json::{Map, Value};

use crate::{config::ActionConfig, item::field_value, source::ensure_success};

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
    command.stdout(Stdio::null());
    let output = crate::source::run_command(&mut command, &label)?;
    ensure_success(&label, &output)
}

fn interpolate(argv: &[String], item: &Map<String, Value>) -> Result<Vec<String>> {
    argv.iter()
        .map(|argument| {
            let Some(path) = field_expression(argument) else {
                return Ok(argument.clone());
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
        })
        .collect()
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
    use crate::config::{Bindings, OnSuccess};

    fn action(command: Vec<&str>) -> ActionConfig {
        ActionConfig {
            name: "check".into(),
            label: "Check".into(),
            icon: String::new(),
            description: String::new(),
            key: Bindings::default(),
            command: Some(command.into_iter().map(str::to_owned).collect()),
            shell: None,
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
}
