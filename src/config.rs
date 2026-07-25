use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub search: SearchConfig,
    pub source: SourceConfig,
    #[serde(default)]
    pub keybindings: Keybindings,
    #[serde(default)]
    pub input: InputConfig,
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
    pub enabled: bool,
    pub down: Bindings,
    pub up: Bindings,
    pub accept: Bindings,
    pub cancel: Bindings,
    pub forward: Bindings,
    pub backward: Bindings,
    pub start: Bindings,
    pub end: Bindings,
    pub delete_word: Bindings,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            enabled: true,
            down: Bindings::new(["down", "ctrl-n"]),
            up: Bindings::new(["up", "ctrl-p"]),
            accept: Bindings::new(["enter"]),
            cancel: Bindings::new(["esc"]),
            forward: Bindings::new(["ctrl-f"]),
            backward: Bindings::new(["ctrl-b"]),
            start: Bindings::new(["ctrl-a"]),
            end: Bindings::new(["ctrl-e"]),
            delete_word: Bindings::new(["ctrl-w"]),
        }
    }
}

impl Keybindings {
    pub fn display_binding<'a>(&self, bindings: &'a Bindings) -> &'a str {
        if self.enabled {
            bindings.label()
        } else {
            "disabled"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings(pub Vec<Binding>);

impl Bindings {
    fn new<const N: usize>(values: [&str; N]) -> Self {
        Self(
            values
                .into_iter()
                .map(|value| Binding::parse(value).expect("default bindings must be valid"))
                .collect(),
        )
    }

    pub fn label(&self) -> &str {
        self.0.first().map_or("disabled", |binding| &binding.label)
    }

    pub fn matches(&self, key: KeyEvent) -> bool {
        self.0
            .iter()
            .any(|binding| binding.code == key.code && binding.modifiers == key.modifiers)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    label: String,
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl Binding {
    fn parse(value: &str) -> Result<Self> {
        let label = value.to_ascii_lowercase();
        let (modifiers, name) = label
            .strip_prefix("ctrl-")
            .map_or((KeyModifiers::NONE, label.as_str()), |name| {
                (KeyModifiers::CONTROL, name)
            });
        let code = match name {
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "enter" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            value => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(character), None) => KeyCode::Char(character),
                    _ => bail!("unsupported keybinding '{value}'"),
                }
            }
        };
        Ok(Self {
            label,
            code,
            modifiers,
        })
    }
}

impl<'de> Deserialize<'de> for Bindings {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Enabled(bool),
            One(String),
            Many(Vec<String>),
        }

        match Value::deserialize(deserializer)? {
            Value::Enabled(false) => Ok(Self(Vec::new())),
            Value::Enabled(true) => Err(serde::de::Error::custom(
                "use a key name or list of key names to enable a binding",
            )),
            Value::One(value) => Binding::parse(&value)
                .map(|binding| Self(vec![binding]))
                .map_err(serde::de::Error::custom),
            Value::Many(values) => values
                .iter()
                .map(|value| Binding::parse(value))
                .collect::<Result<Vec<_>>>()
                .map(Self)
                .map_err(serde::de::Error::custom),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct InputConfig {
    pub vim: bool,
    pub start_mode: InputMode,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            vim: true,
            start_mode: InputMode::Insert,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    Normal,
    #[default]
    Insert,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemConfig {
    #[serde(default)]
    pub border: bool,
    #[serde(default = "default_padding")]
    pub padding: u16,
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
        Self::parse_layered(None, input)
    }

    pub fn parse_layered(global: Option<&str>, palette: &str) -> Result<Self> {
        let mut value = match global {
            Some(global) => {
                toml::from_str(global).context("invalid global Vellum configuration")?
            }
            None => toml::Value::Table(Default::default()),
        };
        let palette = toml::from_str(palette).context("invalid Vellum palette")?;
        merge(&mut value, palette);
        let config: Self = value
            .try_into()
            .context("invalid merged Vellum configuration")?;
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

fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

const fn default_true() -> bool {
    true
}

const fn default_padding() -> u16 {
    1
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
    fn cfg_001_parses_minimal_config_with_defaults() {
        let config = Config::parse(MINIMAL).unwrap();

        assert!(config.search.enabled);
        assert_eq!(config.keybindings.accept.label(), "enter");
        assert_eq!(config.keybindings.down.0[1].label, "ctrl-n");
        assert!(config.input.vim);
        assert_eq!(config.input.start_mode, InputMode::Insert);
        assert_eq!(config.item.padding, 1);
        assert_eq!(config.theme.selection_background, "cyan");
    }

    #[test]
    fn cfg_002_parses_token_rules_and_styled_segments() {
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
    fn cfg_003_rejects_animation_without_frames() {
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
    fn cfg_004_rejects_unsafe_animation_rate() {
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

    }

    #[test]
    fn cfg_005_rejects_invalid_binding() {
        let binding =
            Config::parse(&format!("{MINIMAL}\n[keybindings]\ncancel = 'quit'")).unwrap_err();
        assert!(format!("{binding:#}").contains("unsupported keybinding"));
    }

    #[test]
    fn cfg_006_merges_global_defaults_under_palette_overrides() {
        let global = r#"
            [search]
            placeholder = "Global"

            [input]
            start_mode = "normal"

            [keybindings]
            down = ["ctrl-j"]

            [item]
            padding = 3
        "#;
        let palette = format!("{MINIMAL}\n[search]\nplaceholder = 'Palette'");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.search.placeholder, "Palette");
        assert_eq!(config.input.start_mode, InputMode::Normal);
        assert_eq!(config.keybindings.down.label(), "ctrl-j");
        assert_eq!(config.item.padding, 3);
    }

    #[test]
    fn cfg_007_allows_individual_bindings_and_all_bindings_to_be_disabled() {
        let config = Config::parse(&format!(
            "{MINIMAL}\n[keybindings]\nenabled = false\ndown = false"
        ))
        .unwrap();

        assert!(!config.keybindings.enabled);
        assert!(config.keybindings.down.0.is_empty());
    }
}
