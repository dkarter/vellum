use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::Theme;

struct Preset {
    id: &'static str,
    label: &'static str,
    background: &'static str,
    foreground: &'static str,
    accent: &'static str,
    border: &'static str,
    insert: &'static str,
    normal: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset {
        id: "tokyo-night",
        label: "Tokyo Night · Night",
        background: "#1a1b26",
        foreground: "#c0caf5",
        accent: "#7aa2f7",
        border: "#565f89",
        insert: "#9ece6a",
        normal: "#e0af68",
    },
    Preset {
        id: "tokyo-storm",
        label: "Tokyo Night · Storm",
        background: "#24283b",
        foreground: "#c0caf5",
        accent: "#7aa2f7",
        border: "#565f89",
        insert: "#9ece6a",
        normal: "#e0af68",
    },
    Preset {
        id: "tokyo-moon",
        label: "Tokyo Night · Moon",
        background: "#222436",
        foreground: "#c8d3f5",
        accent: "#82aaff",
        border: "#636da6",
        insert: "#c3e88d",
        normal: "#ffc777",
    },
    Preset {
        id: "tokyo-day",
        label: "Tokyo Night · Day",
        background: "#e1e2e7",
        foreground: "#3760bf",
        accent: "#2e7de9",
        border: "#a8aecb",
        insert: "#587539",
        normal: "#8c6c3e",
    },
    Preset {
        id: "catppuccin-latte",
        label: "Catppuccin · Latte",
        background: "#eff1f5",
        foreground: "#4c4f69",
        accent: "#1e66f5",
        border: "#9ca0b0",
        insert: "#40a02b",
        normal: "#df8e1d",
    },
    Preset {
        id: "catppuccin-frappe",
        label: "Catppuccin · Frappé",
        background: "#303446",
        foreground: "#c6d0f5",
        accent: "#8caaee",
        border: "#737994",
        insert: "#a6d189",
        normal: "#e5c890",
    },
    Preset {
        id: "catppuccin-macchiato",
        label: "Catppuccin · Macchiato",
        background: "#24273a",
        foreground: "#cad3f5",
        accent: "#8aadf4",
        border: "#6e738d",
        insert: "#a6da95",
        normal: "#eed49f",
    },
    Preset {
        id: "catppuccin-mocha",
        label: "Catppuccin · Mocha",
        background: "#1e1e2e",
        foreground: "#cdd6f4",
        accent: "#89b4fa",
        border: "#585b70",
        insert: "#a6e3a1",
        normal: "#f9e2af",
    },
    Preset {
        id: "dracula",
        label: "Dracula · Classic",
        background: "#282a36",
        foreground: "#f8f8f2",
        accent: "#bd93f9",
        border: "#6272a4",
        insert: "#50fa7b",
        normal: "#f1fa8c",
    },
    Preset {
        id: "dracula-soft",
        label: "Dracula · Soft",
        background: "#282a36",
        foreground: "#f8f8f2",
        accent: "#ff79c6",
        border: "#6272a4",
        insert: "#50fa7b",
        normal: "#f1fa8c",
    },
    Preset {
        id: "gruvbox-dark",
        label: "Gruvbox · Dark",
        background: "#282828",
        foreground: "#ebdbb2",
        accent: "#d79921",
        border: "#665c54",
        insert: "#b8bb26",
        normal: "#fabd2f",
    },
    Preset {
        id: "gruvbox-light",
        label: "Gruvbox · Light",
        background: "#fbf1c7",
        foreground: "#3c3836",
        accent: "#b57614",
        border: "#a89984",
        insert: "#79740e",
        normal: "#b57614",
    },
    Preset {
        id: "nord",
        label: "Nord",
        background: "#2e3440",
        foreground: "#d8dee9",
        accent: "#88c0d0",
        border: "#4c566a",
        insert: "#a3be8c",
        normal: "#ebcb8b",
    },
    Preset {
        id: "rose-pine",
        label: "Rosé Pine · Main",
        background: "#191724",
        foreground: "#e0def4",
        accent: "#c4a7e7",
        border: "#6e6a86",
        insert: "#9ccfd8",
        normal: "#f6c177",
    },
    Preset {
        id: "rose-pine-moon",
        label: "Rosé Pine · Moon",
        background: "#232136",
        foreground: "#e0def4",
        accent: "#c4a7e7",
        border: "#6e6a86",
        insert: "#9ccfd8",
        normal: "#f6c177",
    },
    Preset {
        id: "rose-pine-dawn",
        label: "Rosé Pine · Dawn",
        background: "#faf4ed",
        foreground: "#575279",
        accent: "#907aa9",
        border: "#9893a5",
        insert: "#56949f",
        normal: "#ea9d34",
    },
    Preset {
        id: "kanagawa-wave",
        label: "Kanagawa · Wave",
        background: "#1f1f28",
        foreground: "#dcd7ba",
        accent: "#7e9cd8",
        border: "#54546d",
        insert: "#98bb6c",
        normal: "#e6c384",
    },
    Preset {
        id: "kanagawa-dragon",
        label: "Kanagawa · Dragon",
        background: "#181616",
        foreground: "#c5c9c5",
        accent: "#8ba4b0",
        border: "#625e5a",
        insert: "#87a987",
        normal: "#c4b28a",
    },
    Preset {
        id: "kanagawa-lotus",
        label: "Kanagawa · Lotus",
        background: "#f2ecbc",
        foreground: "#545464",
        accent: "#4d699b",
        border: "#9e9b93",
        insert: "#6f894e",
        normal: "#c18a45",
    },
];

pub fn items() -> Vec<Map<String, Value>> {
    PRESETS.iter().map(|preset| {
        let family = preset.label.split('·').next().unwrap_or(preset.label).trim();
        json!({"id":preset.id,"name":preset.label,"family":family,"background":preset.background,"foreground":preset.foreground,"accent":preset.accent,"insert":preset.insert,"normal":preset.normal})
            .as_object().unwrap().clone()
    }).collect()
}

pub fn theme(id: &str) -> Option<Theme> {
    let preset = PRESETS.iter().find(|preset| preset.id == id)?;
    Some(Theme {
        background: preset.background.into(),
        foreground: preset.foreground.into(),
        selection_foreground: preset.background.into(),
        selection_background: preset.accent.into(),
        border: preset.border.into(),
        mode_foreground: preset.background.into(),
        insert_mode_background: preset.insert.into(),
        normal_mode_background: preset.normal.into(),
    })
}

pub fn save(path: &Path, id: &str) -> Result<()> {
    let theme = theme(id).with_context(|| format!("unknown theme '{id}'"))?;
    let original = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let mut doc = DocumentMut::from_str(&original).context("invalid global configuration")?;
    if !doc.as_table().get("theme").is_some_and(Item::is_table) {
        doc.as_table_mut()
            .insert("theme", Item::Table(Table::new()));
    }
    let fields = [
        ("foreground", &theme.foreground),
        ("background", &theme.background),
        ("selection_foreground", &theme.selection_foreground),
        ("selection_background", &theme.selection_background),
        ("border", &theme.border),
        ("mode_foreground", &theme.mode_foreground),
        ("insert_mode_background", &theme.insert_mode_background),
        ("normal_mode_background", &theme.normal_mode_background),
    ];
    for (key, color) in fields {
        doc["theme"][key] = value(color.as_str());
    }
    let parent = path
        .parent()
        .context("global config has no parent directory")?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("refusing to replace symlink {}", path.display());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(doc.to_string().as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal_018_theme_catalog_has_modern_variants() {
        let items = items();
        assert!(items.len() >= 16);
        for item in &items {
            assert!(theme(item["id"].as_str().unwrap()).is_some());
        }
        assert_ne!(theme("tokyo-night"), theme("tokyo-day"));
        for (id, example) in [
            (
                "tokyo-night",
                include_str!("../examples/themes/tokyo-night.toml"),
            ),
            (
                "catppuccin-mocha",
                include_str!("../examples/themes/catppuccin-mocha.toml"),
            ),
            ("dracula", include_str!("../examples/themes/dracula.toml")),
            (
                "gruvbox-dark",
                include_str!("../examples/themes/gruvbox-dark.toml"),
            ),
            ("nord", include_str!("../examples/themes/nord.toml")),
        ] {
            let example: toml::Value = toml::from_str(example).unwrap();
            let expected: Theme = example.get("theme").unwrap().clone().try_into().unwrap();
            assert_eq!(
                theme(id),
                Some(expected),
                "{id} drifted from its theme example"
            );
        }
    }

    #[test]
    fn pal_019_theme_save_creates_and_preserves_unrelated_configuration() {
        let root = std::env::temp_dir().join(format!("vellum-theme-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        fs::write(
            &path,
            "# keep this comment\n[input]\nvim = false\n\n[theme]\nforeground = 'old'\n",
        )
        .unwrap();
        save(&path, "catppuccin-latte").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# keep this comment"));
        assert!(content.contains("vim = false"));
        assert!(content.contains("#eff1f5"));
        fs::remove_file(&path).unwrap();
        save(&path, "tokyo-night").unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("#1a1b26"));
        fs::remove_dir_all(root).unwrap();
    }
}
