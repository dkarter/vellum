use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub search: SearchConfig,
    pub source: SourceConfig,
    #[serde(default)]
    pub keybindings: Keybindings,
    pub item: ItemConfig,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    pub enabled: bool,
    pub placeholder: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placeholder: "Search...".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub cmd: String,
    #[serde(default)]
    pub refresh_ms: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Keybindings {
    pub down: String,
    pub up: String,
    pub accept: String,
    pub cancel: String,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            down: "down".into(),
            up: "up".into(),
            accept: "enter".into(),
            cancel: "esc".into(),
        }
    }
}

impl Keybindings {
    pub fn normalized(mut self) -> Self {
        self.down.make_ascii_lowercase();
        self.up.make_ascii_lowercase();
        self.accept.make_ascii_lowercase();
        self.cancel.make_ascii_lowercase();
        self
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemConfig {
    #[serde(default)]
    pub border: bool,
    #[serde(default)]
    pub tokens: Vec<TokenDefinition>,
    pub template: Vec<Vec<SegmentConfig>>,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TokenDefinition {
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub when: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub fg: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub animation_fps: Option<u16>,
    #[serde(default)]
    pub animation_frames: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SegmentConfig {
    Token(String),
    Styled(StyledSegment),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyledSegment {
    pub token: String,
    #[serde(default)]
    pub fg: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default = "default_true")]
    pub searchable: bool,
    #[serde(default)]
    pub align: Alignment,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub foreground: String,
    pub background: String,
    pub selection_foreground: String,
    pub selection_background: String,
    pub border: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: "reset".into(),
            background: "reset".into(),
            selection_foreground: "black".into(),
            selection_background: "cyan".into(),
            border: "dark_gray".into(),
        }
    }
}

impl Config {
    pub fn parse(input: &str) -> Result<Self> {
        let config: Self = toml::from_str(input).context("invalid Vellum configuration")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.source.cmd.trim().is_empty() {
            bail!("source.cmd cannot be empty");
        }
        if self.item.template.is_empty() || self.item.template.iter().any(Vec::is_empty) {
            bail!("item.template must contain non-empty rows");
        }
        if self.item.value.trim().is_empty() {
            bail!("item.value cannot be empty");
        }
        for binding in [
            &self.keybindings.down,
            &self.keybindings.up,
            &self.keybindings.accept,
            &self.keybindings.cancel,
        ] {
            validate_binding(binding)?;
        }
        for token in &self.item.tokens {
            if token.name.trim().is_empty() || token.source.trim().is_empty() {
                bail!("item token name and source cannot be empty");
            }
            if token.animation_fps == Some(0) {
                bail!("animation_fps must be greater than zero");
            }
            if token.animation_fps.is_some_and(|fps| fps > 1_000) {
                bail!("animation_fps cannot exceed 1000");
            }
            if token.animation_fps.is_some() && token.animation_frames.is_empty() {
                bail!("animated token '{}' has no animation_frames", token.name);
            }
        }
        Ok(())
    }
}

fn validate_binding(binding: &str) -> Result<()> {
    let normalized = binding.to_ascii_lowercase();
    let name = normalized.strip_prefix("ctrl-").unwrap_or(&normalized);
    let named = matches!(
        name,
        "up" | "down" | "left" | "right" | "enter" | "esc" | "escape" | "backspace"
    );
    if named || name.chars().count() == 1 {
        Ok(())
    } else {
        bail!("unsupported keybinding '{binding}'")
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
        [source]
        cmd = "herdr agents --json"

        [item]
        template = [["$name"]]
        value = "$id"
    "#;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let config = Config::parse(MINIMAL).unwrap();

        assert!(config.search.enabled);
        assert_eq!(config.keybindings.accept, "enter");
        assert_eq!(config.theme.selection_background, "cyan");
    }

    #[test]
    fn parses_token_rules_and_styled_segments() {
        let config = Config::parse(&format!(
            "{MINIMAL}\n{}",
            r#"
            [[item.tokens]]
            name = "state_icon"
            source = "state"
            when = ["running"]
            animation_fps = 3
            animation_frames = [".", "o"]
            "#
        ))
        .unwrap();

        assert_eq!(config.item.tokens[0].when, ["running"]);
        assert_eq!(config.item.tokens[0].animation_frames, [".", "o"]);
    }

    #[test]
    fn rejects_animation_without_frames() {
        let error = Config::parse(&format!(
            "{MINIMAL}\n{}",
            r#"
            [[item.tokens]]
            name = "state_icon"
            source = "state"
            animation_fps = 3
            "#
        ))
        .unwrap_err();

        assert!(error.to_string().contains("has no animation_frames"));
    }

    #[test]
    fn rejects_unsafe_animation_rate_and_invalid_binding() {
        let animation = Config::parse(&format!(
            "{MINIMAL}\n{}",
            r#"
            [[item.tokens]]
            name = "icon"
            source = "state"
            animation_fps = 1001
            animation_frames = ["."]
            "#
        ))
        .unwrap_err();
        assert!(animation.to_string().contains("cannot exceed 1000"));

        let binding =
            Config::parse(&format!("{MINIMAL}\n[keybindings]\ncancel = 'quit'")).unwrap_err();
        assert!(binding.to_string().contains("unsupported keybinding"));
    }
}
