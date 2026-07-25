use std::process::{Command, Output};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

use crate::config::SourceConfig;

pub type SourceItem = Map<String, Value>;

pub fn run(source: &SourceConfig) -> Result<Vec<SourceItem>> {
    if let Some(builtin) = source.builtin {
        return builtin.run();
    }
    let command = source
        .cmd
        .as_deref()
        .context("source has neither cmd nor builtin")?;
    let stdout = command_output(
        Command::new("sh").args(["-c", command]),
        &format!("source command: {command}"),
    )?;
    parse(&stdout)
}

pub(crate) fn command_output(command: &mut Command, label: &str) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("failed to run {label}"))?;
    ensure_success(label, &output)?;
    String::from_utf8(output.stdout).with_context(|| format!("{label} output is not UTF-8"))
}

pub(crate) fn ensure_success(label: &str, output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    bail!("{label} exited with {}: {stderr}", output.status)
}

pub fn parse(input: &str) -> Result<Vec<SourceItem>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    if input.starts_with('[') {
        let values: Vec<Value> =
            serde_json::from_str(input).context("invalid JSON source array")?;
        return values.into_iter().map(expect_object).collect();
    }

    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value = serde_json::from_str(line).context("invalid NDJSON source item")?;
            expect_object(value)
        })
        .collect()
}

fn expect_object(value: Value) -> Result<SourceItem> {
    match value {
        Value::Object(item) => Ok(item),
        _ => bail!("each source item must be a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn src_001_parses_json_array() {
        let items = parse(r#"[{"id":1,"name":"one"},{"id":2,"name":"two"}]"#).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["name"], "two");
    }

    #[test]
    fn src_002_parses_ndjson_and_ignores_blank_lines() {
        let items = parse("{\"id\":1}\n\n{\"id\":2}\n").unwrap();

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn src_003_rejects_non_object_items() {
        assert!(
            parse("[1]")
                .unwrap_err()
                .to_string()
                .contains("JSON object")
        );
    }

    #[test]
    fn src_004_runs_successful_source_command() {
        let items = run(&command_source("printf '%s' '[{\"id\":\"one\"}]'")).unwrap();

        assert_eq!(items[0]["id"], "one");
    }

    #[test]
    fn src_005_reports_failed_source_command() {
        let error = run(&command_source("printf 'broken source' >&2; exit 7")).unwrap_err();

        assert!(error.to_string().contains("broken source"));
    }

    fn command_source(command: &str) -> SourceConfig {
        SourceConfig {
            cmd: Some(command.into()),
            builtin: None,
            refresh_ms: 0,
        }
    }
}
