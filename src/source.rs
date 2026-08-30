use std::{
    fs,
    io::{self, BufRead, BufReader, Read},
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::{Context, Result, bail};
use json_comments::StripComments;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::config::SourceConfig;

pub type SourceItem = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdinSource {
    pub mode: StdinMode,
    pub fields: Vec<FieldMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinMode {
    Auto,
    Json,
    Lines { field: String },
    Jq { filter: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMapping {
    pub target: String,
    pub source: String,
}

pub fn run(source: &SourceConfig) -> Result<Vec<SourceItem>> {
    if let Some(builtin) = source.builtin {
        return builtin.run();
    }
    if let Some(path) = &source.file {
        return load_file(path);
    }
    if source.stdin {
        return run_stdin(&StdinSource {
            mode: StdinMode::Json,
            fields: Vec::new(),
        });
    }
    let command = source
        .cmd
        .as_deref()
        .context("source has no cmd, builtin, file, or stdin")?;
    let stdout = command_output(
        Command::new("sh").args(["-c", command]),
        &format!("source command: {command}"),
    )?;
    parse(&stdout)
}

pub fn run_stdin(source: &StdinSource) -> Result<Vec<SourceItem>> {
    let items = match &source.mode {
        StdinMode::Auto => {
            parse_auto(io::stdin().lock()).context("failed to read source from stdin")
        }
        StdinMode::Json => {
            parse_reader(io::stdin().lock()).context("failed to read source from stdin")
        }
        StdinMode::Lines { field } => parse_lines(io::stdin().lock(), field),
        StdinMode::Jq { filter } => run_jq(filter),
    }
    .context("failed to parse stdin source")?;
    apply_field_mappings(items, &source.fields)
}

fn parse_auto(reader: impl Read) -> Result<Vec<SourceItem>> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .context("failed to read stdin source")?
            == 0
        {
            return Ok(Vec::new());
        }
        if !line.trim().is_empty() {
            break;
        }
    }

    if line.trim_start().starts_with('[') {
        return parse_reader(io::Cursor::new(line.into_bytes()).chain(reader));
    }
    if let Ok(Value::Object(first)) = serde_json::from_str(line.trim()) {
        let mut items = vec![first];
        loop {
            line.clear();
            if reader
                .read_line(&mut line)
                .context("failed to read stdin source")?
                == 0
            {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let value = serde_json::from_str(&line).context("invalid NDJSON source item")?;
            items.push(expect_object(value)?);
        }
        return Ok(items);
    }

    let mut items = vec![plain_line_item(line.trim_end_matches(['\r', '\n']))];
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .context("failed to read stdin source")?
            == 0
        {
            break;
        }
        let value = line.trim_end_matches(['\r', '\n']);
        if !value.is_empty() {
            items.push(plain_line_item(value));
        }
    }
    Ok(items)
}

fn plain_line_item(line: &str) -> SourceItem {
    ["value", "name", "path"]
        .into_iter()
        .map(|field| (field.to_owned(), Value::String(line.to_owned())))
        .collect()
}

pub fn load_json(path: &Path) -> Result<Vec<SourceItem>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to read stdin source cache {}", path.display()))?;
    parse_reader(file)
        .with_context(|| format!("failed to parse stdin source cache {}", path.display()))
}

fn run_jq(filter: &str) -> Result<Vec<SourceItem>> {
    let mut child = Command::new("jq")
        .args(["-c", filter])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to run stdin jq filter")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture stdin jq filter output")?;
    let items = parse_reader(stdout);
    let status = child.wait().context("failed to wait for stdin jq filter")?;
    if !status.success() {
        bail!("stdin jq filter exited with {status}");
    }
    items
}

fn parse_reader(reader: impl Read) -> Result<Vec<SourceItem>> {
    let mut reader = BufReader::new(reader);
    loop {
        let buffer = reader.fill_buf().context("failed to read JSON source")?;
        let Some(index) = buffer.iter().position(|byte| !byte.is_ascii_whitespace()) else {
            let consumed = buffer.len();
            if consumed == 0 {
                return Ok(Vec::new());
            }
            reader.consume(consumed);
            continue;
        };
        reader.consume(index);
        break;
    }

    if reader
        .fill_buf()
        .context("failed to read JSON source")?
        .first()
        == Some(&b'[')
    {
        serde_json::from_reader(reader)
            .context("invalid JSON source array; each source item must be a JSON object")
    } else {
        let mut items = Vec::new();
        let mut line = String::new();
        loop {
            line.clear();
            if reader
                .read_line(&mut line)
                .context("failed to read JSON source")?
                == 0
            {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            let value = serde_json::from_str(&line).context("invalid NDJSON source item")?;
            items.push(expect_object(value)?);
        }
        Ok(items)
    }
}

fn parse_lines(reader: impl Read, field: &str) -> Result<Vec<SourceItem>> {
    BufReader::new(reader)
        .lines()
        .filter_map(|line| match line {
            Ok(line) if line.is_empty() => None,
            result => Some(result),
        })
        .map(|line| {
            let mut item = SourceItem::new();
            item.insert(
                field.to_owned(),
                Value::String(line.context("failed to read source from stdin")?),
            );
            Ok(item)
        })
        .collect()
}

fn apply_field_mappings(
    mut items: Vec<SourceItem>,
    mappings: &[FieldMapping],
) -> Result<Vec<SourceItem>> {
    for (index, item) in items.iter_mut().enumerate() {
        let values = mappings
            .iter()
            .map(|mapping| {
                crate::item::field_value(item, &mapping.source)
                    .cloned()
                    .with_context(|| {
                        format!(
                            "stdin item {} has no field '{}' for mapping '{}={}'",
                            index + 1,
                            mapping.source,
                            mapping.target,
                            mapping.source
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        for (mapping, value) in mappings.iter().zip(values) {
            item.insert(mapping.target.clone(), value);
        }
    }
    Ok(items)
}

fn load_file(path: &Path) -> Result<Vec<SourceItem>> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    if !matches!(extension, Some("json" | "jsonc" | "yaml" | "yml" | "toml")) {
        bail!(
            "unsupported source file extension for {}; expected .json, .jsonc, .yaml, .yml, or .toml",
            path.display()
        );
    }
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let items = match extension {
        Some("json") => parse(&input),
        Some("jsonc") => parse_jsonc(&input),
        Some("yaml" | "yml") => noyalib::from_str::<Vec<SourceItem>>(&input)
            .context("expected a top-level YAML sequence of mappings"),
        Some("toml") => toml::from_str::<TomlItems>(&input)
            .map(|source| source.items)
            .context("expected TOML [[items]] array-of-table entries"),
        _ => unreachable!("extension was validated above"),
    };
    items.with_context(|| format!("failed to parse source file {}", path.display()))
}

fn parse_jsonc(input: &str) -> Result<Vec<SourceItem>> {
    let mut stripped = String::with_capacity(input.len());
    StripComments::new(input.as_bytes())
        .read_to_string(&mut stripped)
        .context("failed to strip JSONC comments")?;
    parse(&stripped)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlItems {
    items: Vec<SourceItem>,
}

pub(crate) fn command_output(command: &mut Command, label: &str) -> Result<String> {
    let output = run_command(command, label)?;
    ensure_success(label, &output)?;
    String::from_utf8(output.stdout).with_context(|| format!("{label} output is not UTF-8"))
}

pub(crate) fn run_command(command: &mut Command, label: &str) -> Result<Output> {
    command
        .output()
        .with_context(|| format!("failed to run {label}"))
}

pub(crate) fn ensure_success(label: &str, output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    bail!("{label} exited with {}: {stderr}", output.status)
}

pub fn parse(input: &str) -> Result<Vec<SourceItem>> {
    parse_reader(input.as_bytes())
}

fn expect_object(value: Value) -> Result<SourceItem> {
    match value {
        Value::Object(item) => Ok(item),
        _ => bail!("each source item must be a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

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

        assert!(error.to_string().contains("status: 7"));
        assert!(error.to_string().contains("broken source"));
    }

    #[test]
    fn src_006_json_files_preserve_command_output_shapes() {
        let array = TestFile::new("json", r#"[{"id":1},{"id":2}]"#);
        let ndjson = TestFile::new("json", "{\"id\":3}\n\n{\"id\":4}\n");

        assert_eq!(run(&file_source(array.path())).unwrap().len(), 2);
        assert_eq!(run(&file_source(ndjson.path())).unwrap()[1]["id"], 4);
    }

    #[test]
    fn src_007_jsonc_files_allow_comments() {
        let file = TestFile::new(
            "jsonc",
            "// first item\n{\"id\":1}\n/* between */\n{\"id\":2} // trailing\n",
        );

        let items = run(&file_source(file.path())).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["id"], 2);
    }

    #[test]
    fn src_008_yaml_files_use_a_sequence_of_mappings() {
        for extension in ["yaml", "yml"] {
            let file = TestFile::new(extension, "- id: one\n  enabled: true\n- id: two\n");
            let items = run(&file_source(file.path())).unwrap();

            assert_eq!(items.len(), 2);
            assert_eq!(items[0]["enabled"], true);
        }
    }

    #[test]
    fn src_009_toml_files_use_an_items_array_of_tables() {
        let file = TestFile::new(
            "toml",
            "[[items]]\nid = 'one'\n\n[[items]]\nid = 'two'\nenabled = true\n",
        );

        let items = run(&file_source(file.path())).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["enabled"], true);
    }

    #[test]
    fn src_010_unsupported_file_extensions_are_actionable() {
        let file = TestFile::new("txt", "[]");

        let error = run(&file_source(file.path())).unwrap_err().to_string();

        assert!(error.contains(&file.path().display().to_string()));
        assert!(error.contains(".json, .jsonc, .yaml, .yml, or .toml"));
    }

    #[test]
    fn src_011_invalid_file_contents_identify_the_expected_shape() {
        let invalid = TestFile::new("yaml", "- [not, a, mapping]\n");
        let wrong_shape = TestFile::new("toml", "name = 'not items'\n");

        let yaml_error = format!("{:#}", run(&file_source(invalid.path())).unwrap_err());
        let toml_error = format!("{:#}", run(&file_source(wrong_shape.path())).unwrap_err());

        assert!(yaml_error.contains(&invalid.path().display().to_string()));
        assert!(yaml_error.contains("top-level YAML sequence of mappings"));
        assert!(toml_error.contains(&wrong_shape.path().display().to_string()));
        assert!(toml_error.contains("TOML [[items]]"));
    }

    #[test]
    fn src_015_refresh_reloads_file_contents() {
        let file = TestFile::new("json", r#"[{"id":"before"}]"#);
        let source = file_source(file.path());
        assert_eq!(run(&source).unwrap()[0]["id"], "before");

        fs::write(file.path(), r#"[{"id":"after"}]"#).unwrap();

        assert_eq!(run(&source).unwrap()[0]["id"], "after");
    }

    #[test]
    fn src_016_standard_input_is_parsed_as_json_source_items() {
        let mut input = &b"{\"id\":1}\n{\"id\":2}\n"[..];

        let items = parse_reader(&mut input).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["id"], 2);
    }

    #[test]
    fn src_018_plain_lines_become_named_source_fields() {
        let items = parse_lines(&b"first\n\nsecond\n"[..], "name").unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["name"], "first");
        assert_eq!(items[1]["name"], "second");
    }

    #[test]
    fn src_019_simple_field_mappings_reshape_json_objects() {
        let items = parse(r#"[{"id":"original","details":{"name":"First"}}]"#).unwrap();
        let mappings = vec![
            FieldMapping {
                target: "id".into(),
                source: "details.name".into(),
            },
            FieldMapping {
                target: "value".into(),
                source: "id".into(),
            },
        ];

        let items = apply_field_mappings(items, &mappings).unwrap();

        assert_eq!(items[0]["id"], "First");
        assert_eq!(items[0]["value"], "original");
    }

    #[test]
    fn src_020_automatic_stdin_accepts_plain_lines_and_structured_json() {
        let lines = parse_auto(&b"  first  \n\nsecond\n"[..]).unwrap();
        let json = parse_auto(&br#"[{"id":1}]"#[..]).unwrap();
        let ndjson = parse_auto(&b"{\"id\":2}\n{\"id\":3}\n"[..]).unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["value"], "  first  ");
        assert_eq!(lines[0]["name"], "  first  ");
        assert_eq!(lines[0]["path"], "  first  ");
        assert_eq!(json[0]["id"], 1);
        assert_eq!(ndjson[1]["id"], 3);
    }

    fn command_source(command: &str) -> SourceConfig {
        SourceConfig {
            cmd: Some(command.into()),
            builtin: None,
            file: None,
            stdin: false,
            refresh_ms: 0,
        }
    }

    fn file_source(path: &Path) -> SourceConfig {
        SourceConfig {
            cmd: None,
            builtin: None,
            file: Some(path.to_owned()),
            stdin: false,
            refresh_ms: 0,
        }
    }

    struct TestFile(PathBuf);

    impl TestFile {
        fn new(extension: &str, contents: &str) -> Self {
            let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "vellum-source-{}-{id}.{extension}",
                std::process::id()
            ));
            fs::write(&path, contents).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
}
