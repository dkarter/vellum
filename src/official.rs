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

    use serde_json::{Map, Value};

    use super::*;
    use crate::{
        builtins,
        config::{Config, SegmentConfig},
        item::render_item,
    };

    const SNAPSHOT: &str = r#"{
      "result":{"snapshot":{
        "workspaces":[{"workspace_id":"w1","number":1,"label":"vellum","focused":true,"agent_status":"working","pane_count":2,"worktree":{"repo_name":"vellum","checkout_path":"/tmp/vellum"}}],
        "agents":[{"workspace_id":"w1","pane_id":"w1:p1","agent":"opencode","agent_status":"working","terminal_title_stripped":"Official palettes","cwd":"/tmp/vellum"}]
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
                .all(|segment| match segment {
                    SegmentConfig::Token(token) => token != "$pane_id",
                    SegmentConfig::Styled(segment) => segment.token != "$pane_id",
                })
        );
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
            .map(|segment| match segment {
                SegmentConfig::Token(token) => token,
                SegmentConfig::Styled(segment) => &segment.token,
            })
            .chain(std::iter::once(&config.item.value));
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
    }
}
