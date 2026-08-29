use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

pub struct Palette {
    pub name: &'static str,
    pub contents: &'static str,
}

pub const PALETTES: &[Palette] = &[
    Palette {
        name: "herdr-workspaces",
        contents: include_str!("../palettes/herdr-workspaces.toml"),
    },
    Palette {
        name: "herdr-agents",
        contents: include_str!("../palettes/herdr-agents.toml"),
    },
    Palette {
        name: "files",
        contents: include_str!("../palettes/files.toml"),
    },
];

#[derive(Debug, PartialEq, Eq)]
pub struct SyncReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub overwritten: Vec<String>,
}

pub fn sync(config_root: &Path, overwrite: bool) -> Result<SyncReport> {
    let directory = config_root.join("palettes");
    fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    let mut report = SyncReport {
        installed: Vec::new(),
        skipped: Vec::new(),
        overwritten: Vec::new(),
    };

    for palette in PALETTES {
        let filename = format!("{}.toml", palette.name);
        let path = directory.join(&filename);
        if overwrite {
            let metadata = fs::symlink_metadata(&path);
            if metadata
                .as_ref()
                .is_ok_and(|value| value.file_type().is_symlink())
            {
                bail!("refusing to overwrite symlink {}", path.display());
            }
            let existed = metadata.is_ok();
            atomic_replace(&path, palette.contents)?;
            if existed {
                report.overwritten.push(filename);
            } else {
                report.installed.push(filename);
            }
            continue;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(palette.contents.as_bytes()) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error)
                        .with_context(|| format!("failed to write {}", path.display()));
                }
                report.installed.push(filename);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                report.skipped.push(filename);
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to create {}", path.display()));
            }
        }
    }

    Ok(report)
}

fn atomic_replace(path: &Path, contents: &str) -> Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("failed to replace {}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use ratatui::{Terminal, backend::TestBackend, style::Color};
    use serde_json::{Map, Value};

    use super::*;
    use crate::{
        app::App,
        builtins,
        config::{Config, SegmentConfig},
        item::{field_value, render_item},
        ui,
    };

    const SNAPSHOT: &str = r#"{
      "result":{"snapshot":{
        "workspaces":[{"workspace_id":"w1","number":1,"label":"vellum","focused":true,"agent_status":"working","pane_count":3,"worktree":{"repo_name":"vellum","checkout_path":"/tmp/vellum"}}],
        "agents":[
          {"workspace_id":"w1","pane_id":"w1:p1","agent":"opencode","agent_status":"working","terminal_title_stripped":"Official palettes","cwd":"/tmp/vellum"},
          {"workspace_id":"w1","pane_id":"w1:p2","agent":"opencode","agent_status":"idle","terminal_title_stripped":"Second OpenCode","cwd":"/tmp/vellum"},
          {"workspace_id":"w1","pane_id":"w1:p3","agent":"future-agent","agent_status":"idle","terminal_title_stripped":"Future agent","cwd":"/tmp/vellum"}
        ]
      }}
    }"#;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vellum-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn pal_006_installs_all_palettes_into_injected_config_root() {
        let root = temp_dir("install");

        let report = sync(&root, false).unwrap();

        assert_eq!(report.installed.len(), PALETTES.len());
        assert!(report.skipped.is_empty());
        for palette in PALETTES {
            let installed = fs::read_to_string(
                root.join("palettes")
                    .join(palette.name)
                    .with_extension("toml"),
            )
            .unwrap();
            assert_eq!(installed, palette.contents);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pal_001_safe_sync_does_not_overwrite_existing_palette() {
        let root = temp_dir("overwrite");
        let palettes = root.join("palettes");
        fs::create_dir_all(&palettes).unwrap();
        let existing = palettes.join("herdr-agents.toml");
        fs::write(&existing, "user edits\n").unwrap();

        let safe = sync(&root, false).unwrap();
        assert_eq!(fs::read_to_string(&existing).unwrap(), "user edits\n");
        assert_eq!(safe.skipped, ["herdr-agents.toml"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pal_002_explicit_overwrite_updates_existing_palette() {
        let root = temp_dir("explicit-overwrite");
        let palettes = root.join("palettes");
        fs::create_dir_all(&palettes).unwrap();
        let existing = palettes.join("herdr-agents.toml");
        fs::write(&existing, "user edits\n").unwrap();

        let overwrite = sync(&root, true).unwrap();
        assert_eq!(
            fs::read_to_string(&existing).unwrap(),
            PALETTES
                .iter()
                .find(|palette| palette.name == "herdr-agents")
                .unwrap()
                .contents
        );
        assert!(overwrite.overwritten.contains(&"herdr-agents.toml".into()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pal_007_every_bundled_palette_parses_as_vellum_configuration() {
        assert!(!PALETTES.is_empty());
        for palette in PALETTES {
            Config::parse(palette.contents)
                .unwrap_or_else(|error| panic!("{} is invalid: {error:#}", palette.name));
        }
    }

    #[test]
    fn pal_008_bundled_palettes_match_builtin_source_contracts() {
        for palette in PALETTES {
            let config = Config::parse(palette.contents).unwrap();
            let item = representative_item(palette.name);
            assert_referenced_fields_exist(palette.name, &config, &item);

            let rendered = render_item(&item, &config.item, 0);
            assert!(
                !rendered.value.is_empty(),
                "{} has an empty value",
                palette.name
            );
            assert!(
                !rendered.search_text.is_empty(),
                "{} has no searchable text",
                palette.name
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn pal_009_explicit_overwrite_refuses_symlink_targets() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink");
        let palettes = root.join("palettes");
        fs::create_dir_all(&palettes).unwrap();
        let target = root.join("outside.toml");
        fs::write(&target, "outside\n").unwrap();
        symlink(&target, palettes.join("files.toml")).unwrap();

        let error = sync(&root, true).unwrap_err();
        assert!(error.to_string().contains("refusing to overwrite symlink"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "outside\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pal_010_agent_pane_id_is_output_only() {
        let palette = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-agents")
            .unwrap();
        let config = Config::parse(palette.contents).unwrap();

        assert_eq!(config.item.value, "$pane_id");
        assert!(
            config
                .item
                .template
                .iter()
                .flatten()
                .all(|segment| segment_token(segment) != "$pane_id")
        );
    }

    #[test]
    fn pal_014_agent_palette_filters_by_lifecycle_state() {
        let palette = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-agents")
            .unwrap();
        let config = Config::parse(palette.contents).unwrap();

        assert_eq!(config.filters.mode.label(), "ctrl-g");
        assert_eq!(config.filters.clear.label(), "a");
        assert_eq!(config.filters.label, "status");
        assert_eq!(
            config
                .filters
                .choices
                .iter()
                .map(|choice| (choice.key.label(), choice.value.as_str()))
                .collect::<Vec<_>>(),
            [
                ("w", "working"),
                ("d", "done"),
                ("i", "idle"),
                ("b", "blocked"),
                ("u", "unknown"),
            ]
        );
        assert_eq!(config.filters.choices[0].icon, "●");
        assert_eq!(config.filters.choices[0].fg.as_deref(), Some("yellow"));
        assert_eq!(config.filters.choices[1].icon, "●");
        assert_eq!(config.filters.choices[1].fg.as_deref(), Some("#89b4fa"));
        assert_eq!(config.filters.choices[2].fg.as_deref(), Some("#a6e3a1"));
        assert_eq!(config.filters.choices[3].fg.as_deref(), Some("#ff6188"));
        assert_eq!(config.filters.choices[2].icon, "○");
        assert_eq!(config.filters.choices[4].icon, "·");
        assert_eq!(config.filters.choices[4].fg.as_deref(), Some("#999999"));
        let item = representative_item("herdr-agents");
        assert!(
            render_item(&item, &config.item, 0)
                .search_text
                .contains("working")
        );
    }

    #[test]
    fn pal_016_workspace_palette_filters_match_agent_lifecycle_states() {
        let workspace = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-workspaces")
            .map(|palette| Config::parse(palette.contents).unwrap())
            .unwrap();
        let agents = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-agents")
            .map(|palette| Config::parse(palette.contents).unwrap())
            .unwrap();

        assert_eq!(workspace.filters, agents.filters);
        assert!(
            workspace
                .filters
                .choices
                .iter()
                .all(|choice| choice.source == "agent_status")
        );
        assert_eq!(
            representative_item("herdr-workspaces")["agent_status"],
            workspace.filters.choices[0].value
        );
        let item = representative_item("herdr-workspaces");
        assert!(
            render_item(&item, &workspace.item, 0)
                .search_text
                .contains("working")
        );
    }

    #[test]
    fn pal_011_file_palette_uses_a_compact_path_layout() {
        let palette = PALETTES
            .iter()
            .find(|palette| palette.name == "files")
            .unwrap();
        let config = Config::parse(palette.contents).unwrap();
        let item = builtins::file_item(Path::new("src/main.rs")).unwrap();
        let rendered = render_item(&item, &config.item, 0);

        assert_eq!(rendered.rows.len(), 1);
        assert_eq!(
            rendered.rows[0]
                .segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<String>(),
            format!(" src{}main.rs", std::path::MAIN_SEPARATOR)
        );
        assert!(rendered.rows[0].segments.last().unwrap().bold);
    }

    #[test]
    fn pal_012_bundled_palettes_identify_their_search_inputs() {
        for palette in PALETTES {
            let config = Config::parse(palette.contents).unwrap();
            assert_ne!(config.search.title, "Vellum", "{}", palette.name);
            assert!(!config.search.title.trim().is_empty(), "{}", palette.name);
        }
    }

    #[test]
    fn pal_013_workspace_palette_uses_an_aligned_two_line_layout() {
        let palette = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-workspaces")
            .unwrap();
        let config = Config::parse(palette.contents).unwrap();
        let item = representative_item(palette.name);
        let rendered = render_item(&item, &config.item, 0);
        let template: Vec<Vec<_>> = config
            .item
            .template
            .iter()
            .map(|row| row.iter().map(segment_token).collect())
            .collect();

        assert_eq!(
            template,
            [
                vec!["$label", "$status_icon", " ", "$agent_status",],
                vec!["󰉋 ", "$checkout_path_display"],
            ]
        );
        assert_eq!(rendered.rows.len(), 2);
        assert_eq!(rendered.rows[0].segments[0].text, "vellum");
        assert_eq!(rendered.rows[0].segments[0].fg.as_deref(), Some("#7aa2f7"));
        assert!(rendered.rows[0].segments[0].bold);
        assert_eq!(rendered.rows[1].segments[0].text, "󰉋 ");
        assert_eq!(
            rendered.rows[0].segments.last().unwrap().align,
            crate::config::Alignment::Right
        );
        assert!(
            rendered.rows[1]
                .segments
                .iter()
                .any(|segment| segment.text == "/tmp/vellum")
        );
        let mut app = App::new(
            vec![item],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            config.search.enabled,
        );
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        terminal
            .draw(|frame| ui::render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(1, 3)].fg,
            Color::Rgb(122, 162, 247)
        );
    }

    #[test]
    fn pal_015_workspace_palette_provides_native_focus_and_removal_actions() {
        let palette = PALETTES
            .iter()
            .find(|palette| palette.name == "herdr-workspaces")
            .unwrap();
        let config = Config::parse(palette.contents).unwrap();

        assert_eq!(config.actions.default.as_deref(), Some("focus"));
        assert_eq!(config.actions.menu.label(), "ctrl-a");
        assert_eq!(config.actions.items[0].icon, "󰍉");
        assert!(!config.actions.items[0].description.is_empty());
        assert_eq!(
            config.actions.items[0].command.as_deref(),
            Some(
                ["herdr", "workspace", "focus", "$workspace_id"]
                    .map(str::to_owned)
                    .as_slice()
            )
        );
        assert_eq!(
            config.actions.items[1].command.as_deref(),
            Some(
                ["hwt", "remove", "--workspace", "$workspace_id"]
                    .map(str::to_owned)
                    .as_slice()
            )
        );
        assert_eq!(
            config.actions.items[1].on_success,
            crate::config::OnSuccess::Refresh
        );
        assert_eq!(config.actions.items[1].when.len(), 2);
        assert!(!config.actions.items[1].is_available(&representative_item(palette.name)));
        let item = representative_item(palette.name);
        let mut regular_checkout = item.clone();
        regular_checkout.insert("worktree".into(), Value::Null);
        for (name, command) in [
            ("open-repository", &["gh", "repo", "view", "--web"][..]),
            ("open-pull-request", &["gh", "pr", "view", "--web"]),
            ("open-pull-request-checks", &["gh", "pr", "checks", "--web"]),
        ] {
            let action = config
                .actions
                .items
                .iter()
                .find(|action| action.name == name)
                .unwrap();
            assert_eq!(action.command.as_deref().unwrap(), command);
            assert_eq!(action.cwd.as_deref(), Some("$checkout_path"));
            assert!(action.is_available(&item));
            assert!(action.is_available(&regular_checkout));
            if name == "open-repository" {
                assert!(action.availability.is_none());
            } else {
                let availability = action.availability.as_ref().unwrap();
                assert_eq!(
                    availability.command,
                    ["gh", "pr", "view", "--json", "number"]
                );
                assert_eq!(availability.cwd.as_deref(), Some("$checkout_path"));
                assert_eq!(availability.cache_ms, 30_000);
                assert_eq!(availability.timeout_ms, 5_000);
            }
        }
    }

    fn representative_item(name: &str) -> Map<String, Value> {
        match name {
            "herdr-workspaces" => builtins::herdr_workspaces(SNAPSHOT).unwrap().remove(0),
            "herdr-agents" => builtins::herdr_agents(SNAPSHOT).unwrap().remove(0),
            "files" => builtins::file_item(Path::new("src/main.rs")).unwrap(),
            _ => panic!("missing representative item for {name}"),
        }
    }

    fn assert_referenced_fields_exist(name: &str, config: &Config, item: &Map<String, Value>) {
        let expressions = config
            .item
            .template
            .iter()
            .flatten()
            .map(segment_token)
            .chain(std::iter::once(config.item.value.as_str()));
        for expression in expressions {
            let Some(field) = expression.strip_prefix('$') else {
                continue;
            };
            let field = field.split('.').next().unwrap_or(field);
            let derived = config.item.tokens.iter().any(|token| token.name == field);
            assert!(
                derived || item.contains_key(field),
                "{name} references missing source field {field}"
            );
        }
        for repeated in config
            .item
            .template
            .iter()
            .flatten()
            .filter_map(SegmentConfig::repeated)
        {
            assert!(
                field_value(item, &repeated.for_each).is_some(),
                "{name} repeats missing source field {}",
                repeated.for_each
            );
        }
        for choice in &config.filters.choices {
            assert!(
                field_value(item, &choice.source).is_some(),
                "{name} filter references missing source field {}",
                choice.source
            );
        }
    }

    fn segment_token(segment: &SegmentConfig) -> &str {
        match segment {
            SegmentConfig::Token(token) => token,
            SegmentConfig::Repeated(segment) => &segment.token,
            SegmentConfig::Styled(segment) => &segment.token,
        }
    }
}
