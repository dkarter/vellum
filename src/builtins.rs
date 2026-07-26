use std::{collections::HashMap, path::Path, process::Command};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::source::{SourceItem, command_output, ensure_success};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinSource {
    HerdrWorkspaces,
    HerdrAgents,
    Files,
}

impl BuiltinSource {
    pub fn run(self) -> Result<Vec<SourceItem>> {
        match self {
            Self::HerdrWorkspaces => run_herdr_workspaces(),
            Self::HerdrAgents => run_herdr_agents(),
            Self::Files => run_files(),
        }
    }
}

fn run_herdr_workspaces() -> Result<Vec<SourceItem>> {
    herdr_workspaces(&command_output(
        Command::new("herdr").args(["api", "snapshot"]),
        "herdr api snapshot",
    )?)
}

fn run_herdr_agents() -> Result<Vec<SourceItem>> {
    herdr_agents(&command_output(
        Command::new("herdr").args(["api", "snapshot"]),
        "herdr api snapshot",
    )?)
}

fn run_files() -> Result<Vec<SourceItem>> {
    let output = Command::new("fd")
        .args(["--type", "f", "--color", "never", "--print0"])
        .output()
        .context("failed to run fd")?;
    ensure_success("fd", &output)?;
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = std::str::from_utf8(path).context("fd returned a non-UTF-8 path")?;
            file_item(Path::new(path))
        })
        .collect()
}

pub fn herdr_workspaces(input: &str) -> Result<Vec<SourceItem>> {
    let snapshot = snapshot(input)?;
    let agents = array(&snapshot, "agents")?;
    let mut agents_by_workspace: HashMap<&str, Vec<&Map<String, Value>>> = HashMap::new();
    for agent in agents {
        let agent = object(agent, "agent")?;
        agents_by_workspace
            .entry(string(agent, "workspace_id")?)
            .or_default()
            .push(agent);
    }
    let pane_cwds: HashMap<_, _> = snapshot
        .get("panes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pane| {
            let pane = pane.as_object()?;
            Some((pane.get("workspace_id")?.as_str()?, pane.get("cwd")?))
        })
        .collect();

    array(&snapshot, "workspaces")?
        .iter()
        .map(|workspace| {
            let workspace = object(workspace, "workspace")?;
            let workspace_id = string(workspace, "workspace_id")?;
            let workspace_agents = agents_by_workspace
                .get(workspace_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let mut summaries: HashMap<&str, Vec<String>> = HashMap::new();
            for agent in workspace_agents {
                let state = normalized_state(agent.get("agent_status").and_then(Value::as_str));
                let name = agent
                    .get("agent")
                    .and_then(Value::as_str)
                    .unwrap_or("agent");
                let title = agent
                    .get("terminal_title_stripped")
                    .and_then(Value::as_str)
                    .filter(|title| !title.is_empty());
                let summary = title.map_or_else(
                    || format!("{} {name}", state_icon(state)),
                    |title| format!("{} {name}: {title}", state_icon(state)),
                );
                summaries.entry(state).or_default().push(summary);
            }
            let state = normalized_state(workspace.get("agent_status").and_then(Value::as_str));
            let worktree = workspace.get("worktree").and_then(Value::as_object);
            let repo_name = worktree.and_then(|value| value.get("repo_name")).cloned();
            let checkout_path = worktree
                .and_then(|value| value.get("checkout_path"))
                .cloned()
                .or_else(|| {
                    workspace_agents
                        .first()
                        .and_then(|agent| agent.get("cwd"))
                        .cloned()
                })
                .or_else(|| pane_cwds.get(workspace_id).copied().cloned());
            let mut item = Map::new();
            copy_required(&mut item, workspace, "workspace_id")?;
            copy_required(&mut item, workspace, "number")?;
            copy_required(&mut item, workspace, "label")?;
            copy_required(&mut item, workspace, "pane_count")?;
            let pane_count = workspace
                .get("pane_count")
                .and_then(Value::as_u64)
                .context("Herdr workspace pane_count is not an integer")?;
            item.insert(
                "pane_label".into(),
                format!(
                    "{pane_count} pane{}",
                    if pane_count == 1 { "" } else { "s" }
                )
                .into(),
            );
            item.insert("agent_status".into(), state.into());
            item.insert(
                "agent_summary".into(),
                if workspace_agents.is_empty() {
                    "○ no active agents".into()
                } else {
                    String::new().into()
                },
            );
            let focused = workspace.get("focused") == Some(&Value::Bool(true));
            item.insert(
                "focus_icon".into(),
                if focused {
                    "▶".into()
                } else {
                    String::new().into()
                },
            );
            item.insert(
                "focus_color".into(),
                if focused { "#7aa2f7" } else { "#c0caf5" }.into(),
            );
            decorate_status(&mut item, state);
            item.insert("repo_name".into(), repo_name.unwrap_or(Value::Null));
            item.insert("checkout_path".into(), checkout_path.unwrap_or(Value::Null));
            for state in ["blocked", "working", "done", "idle", "unknown"] {
                item.insert(
                    format!("{state}_agents"),
                    summaries
                        .get(state)
                        .map(|values| values.join(" · "))
                        .unwrap_or_default()
                        .into(),
                );
            }
            Ok(item)
        })
        .collect()
}

pub fn herdr_agents(input: &str) -> Result<Vec<SourceItem>> {
    let snapshot = snapshot(input)?;
    let labels: HashMap<_, _> = array(&snapshot, "workspaces")?
        .iter()
        .map(|workspace| {
            let workspace = object(workspace, "workspace")?;
            Ok((
                string(workspace, "workspace_id")?,
                string(workspace, "label")?,
            ))
        })
        .collect::<Result<_>>()?;

    array(&snapshot, "agents")?
        .iter()
        .map(|agent| {
            let agent = object(agent, "agent")?;
            let workspace_id = string(agent, "workspace_id")?;
            let mut item = Map::new();
            for key in [
                "pane_id",
                "agent",
                "terminal_title_stripped",
                "cwd",
                "workspace_id",
            ] {
                copy_required(&mut item, agent, key)?;
            }
            item.insert(
                "workspace_label".into(),
                labels
                    .get(workspace_id)
                    .copied()
                    .with_context(|| {
                        format!("Herdr agent references unknown workspace {workspace_id}")
                    })?
                    .into(),
            );
            let state = normalized_state(agent.get("agent_status").and_then(Value::as_str));
            item.insert("agent_status".into(), state.into());
            decorate_status(&mut item, state);
            Ok(item)
        })
        .collect()
}

pub fn file_item(path: &Path) -> Result<SourceItem> {
    let path_text = path
        .to_str()
        .context("file path is not valid UTF-8")?
        .to_owned();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("file name is not valid UTF-8")?;
    let parent = path.parent().and_then(Path::to_str).unwrap_or_default();
    let parent_prefix = if parent.is_empty() || parent.ends_with(std::path::MAIN_SEPARATOR) {
        parent.to_owned()
    } else {
        format!("{parent}{}", std::path::MAIN_SEPARATOR)
    };
    let (icon, color) = file_icon(name);
    Ok(Map::from_iter([
        ("path".into(), path_text.into()),
        ("name".into(), name.into()),
        ("parent".into(), parent.into()),
        ("parent_prefix".into(), parent_prefix.into()),
        ("icon".into(), icon.into()),
        ("icon_color".into(), color.into()),
    ]))
}

fn copy_required(target: &mut SourceItem, source: &SourceItem, key: &str) -> Result<()> {
    target.insert(
        key.into(),
        source
            .get(key)
            .with_context(|| format!("Herdr entry is missing {key}"))?
            .clone(),
    );
    Ok(())
}

fn decorate_status(item: &mut SourceItem, state: &str) {
    item.insert("status_icon".into(), state_icon(state).into());
    item.insert("status_color".into(), state_color(state).into());
}

fn snapshot(input: &str) -> Result<Map<String, Value>> {
    let value: Value = serde_json::from_str(input).context("invalid Herdr JSON")?;
    value
        .pointer("/result/snapshot")
        .and_then(Value::as_object)
        .cloned()
        .context("Herdr response is missing result.snapshot")
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("Herdr snapshot is missing {key}"))
}

fn object<'a>(value: &'a Value, kind: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .with_context(|| format!("Herdr {kind} entry is not an object"))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Herdr entry is missing {key}"))
}

fn normalized_state(state: Option<&str>) -> &'static str {
    match state {
        Some("blocked") => "blocked",
        Some("working") => "working",
        Some("done") => "done",
        Some("idle") => "idle",
        _ => "unknown",
    }
}

fn state_icon(state: &str) -> &'static str {
    match state {
        "blocked" => "!",
        "working" => "●",
        "done" | "idle" => "✓",
        _ => "○",
    }
}

fn state_color(state: &str) -> &'static str {
    match state {
        "blocked" => "#f7768e",
        "working" => "#7aa2f7",
        "done" => "#e0af68",
        "idle" => "#9ece6a",
        _ => "#565f89",
    }
}

// Compact categories adapted from Snacks.nvim's nvim-web-devicons fallback.
fn file_icon(name: &str) -> (&'static str, &'static str) {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => return ("", "#DEA584"),
        "dockerfile" | "containerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
            return ("󰡨", "#458EE6");
        }
        "makefile" | "gnumakefile" => return ("", "#6D8086"),
        "package.json" | "package-lock.json" => return ("", "#E8274B"),
        "readme" | "readme.md" => return ("󰂺", "#EDEDED"),
        _ => {}
    }
    let extension = lower
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension);
    match extension {
        "rs" => ("", "#DEA584"),
        "ex" | "exs" | "eex" | "heex" => ("", "#A074C4"),
        "js" | "mjs" | "cjs" => ("", "#CBCB41"),
        "jsx" => ("", "#20C2E3"),
        "ts" | "mts" | "cts" => ("", "#519ABA"),
        "tsx" => ("", "#1354BF"),
        "lua" => ("", "#51A0CF"),
        "py" | "pyi" | "pyw" => ("", "#FFBC03"),
        "rb" | "rake" | "gemspec" => ("", "#701516"),
        "go" => ("", "#00ADD8"),
        "java" | "jar" => ("", "#CC3E44"),
        "c" => ("", "#599EFF"),
        "cc" | "cpp" | "cxx" | "hpp" => ("", "#519ABA"),
        "sh" | "bash" | "zsh" | "fish" => ("", "#89E051"),
        "html" | "htm" => ("", "#E44D26"),
        "css" => ("", "#663399"),
        "scss" | "sass" => ("", "#F55385"),
        "vue" => ("", "#8DC149"),
        "svelte" => ("", "#FF3E00"),
        "md" | "markdown" | "mdx" => ("", "#DDDDDD"),
        "toml" => ("", "#9C4221"),
        "json" | "jsonc" | "json5" => ("", "#CBCB41"),
        "yaml" | "yml" => ("", "#6D8086"),
        "xml" => ("󰗀", "#E37933"),
        "sql" | "db" | "sqlite" | "sqlite3" => ("", "#DAD8D8"),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" => ("", "#A074C4"),
        "svg" | "svgz" => ("󰜡", "#FFB13B"),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => ("", "#00AFFF"),
        "mp4" | "mov" | "mkv" | "avi" | "webm" => ("", "#FD971F"),
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => ("", "#ECA517"),
        "pdf" => ("", "#B30B00"),
        "lock" => ("", "#BBBBBB"),
        _ => ("󰈔", "#6D8086"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SNAPSHOT: &str = r#"{
      "id":"cli:api:snapshot",
      "result":{"type":"session_snapshot","snapshot":{
        "workspaces":[
          {"workspace_id":"w1","number":1,"label":"vellum","focused":true,"agent_status":"working","pane_count":2,"tab_count":1,"worktree":{"repo_name":"vellum","checkout_path":"/tmp/vellum","is_linked_worktree":false}},
          {"workspace_id":"w2","number":2,"label":"docs","focused":false,"agent_status":"unknown","pane_count":1,"tab_count":1}
        ],
        "agents":[
          {"workspace_id":"w1","pane_id":"w1:p1","agent":"opencode","agent_status":"working","terminal_title_stripped":"Official palettes","cwd":"/tmp/vellum","focused":true},
          {"workspace_id":"w1","pane_id":"w1:p2","agent":"claude","agent_status":"idle","terminal_title_stripped":"Reviewing tests","cwd":"/tmp/vellum","focused":false}
        ]
      }}
    }"#;

    #[test]
    fn pal_003_herdr_workspaces_include_active_agent_states() {
        let items = herdr_workspaces(SNAPSHOT).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["workspace_id"], "w1");
        assert_eq!(items[0]["focus_icon"], "▶");
        assert_eq!(items[0]["focus_color"], "#7aa2f7");
        assert_eq!(items[1]["focus_color"], "#c0caf5");
        assert_eq!(items[0]["working_agents"], "● opencode: Official palettes");
        assert_eq!(items[0]["idle_agents"], "✓ claude: Reviewing tests");
        assert_eq!(items[1]["agent_summary"], "○ no active agents");
    }

    #[test]
    fn pal_004_herdr_agents_include_status_and_selection_id() {
        let items = herdr_agents(SNAPSHOT).unwrap();

        assert_eq!(items[0]["pane_id"], "w1:p1");
        assert_eq!(items[0]["workspace_label"], "vellum");
        assert_eq!(items[0]["status_icon"], "●");
        assert_eq!(items[1]["status_icon"], "✓");
    }

    #[test]
    fn pal_005_fd_file_items_have_colorful_nerd_font_icons() {
        let rust = file_item(Path::new("src/main.rs")).unwrap();
        assert_eq!(rust["path"], "src/main.rs");
        assert_eq!(rust["name"], "main.rs");
        assert_eq!(rust["parent"], "src");
        assert_eq!(
            rust["parent_prefix"],
            format!("src{}", std::path::MAIN_SEPARATOR)
        );
        assert_eq!(rust["icon"], "");
        assert_eq!(rust["icon_color"], "#DEA584");

        let image = file_item(Path::new("screenshots/hero.png")).unwrap();
        assert_eq!(image["icon"], "");

        let unknown = file_item(Path::new("odd name.unknown")).unwrap();
        assert_eq!(unknown["icon"], "󰈔");
        assert_eq!(unknown["path"], json!("odd name.unknown"));
    }
}
