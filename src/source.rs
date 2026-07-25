use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

pub type SourceItem = Map<String, Value>;

pub fn run(command: &str) -> Result<Vec<SourceItem>> {
    let output = Command::new("sh")
        .args(["-c", command])
        .output()
        .with_context(|| format!("failed to run source command: {command}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        bail!("source command exited with {}: {stderr}", output.status);
    }

    parse(&String::from_utf8(output.stdout).context("source output is not UTF-8")?)
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
    fn parses_json_array() {
        let items = parse(r#"[{"id":1,"name":"one"},{"id":2,"name":"two"}]"#).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["name"], "two");
    }

    #[test]
    fn parses_ndjson_and_ignores_blank_lines() {
        let items = parse("{\"id\":1}\n\n{\"id\":2}\n").unwrap();

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn rejects_non_object_items() {
        assert!(
            parse("[1]")
                .unwrap_err()
                .to_string()
                .contains("JSON object")
        );
    }
}
