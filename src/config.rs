use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use crate::builtins::BuiltinSource;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub search: SearchConfig,
    pub source: SourceConfig,
    #[serde(default)]
    pub keybindings: Keybindings,
    #[serde(default)]
    pub filters: FilterConfig,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub frecency: FrecencyConfig,
    #[serde(default)]
    pub actions: ActionsConfig,
    pub item: ItemConfig,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ActionsConfig {
    pub default: Option<String>,
    pub menu: Bindings,
    pub items: Vec<ActionConfig>,
}

impl Default for ActionsConfig {
    fn default() -> Self {
        Self {
            default: None,
            menu: Bindings::new(["ctrl-a"]),
            items: Vec::new(),
        }
    }
}

impl ActionsConfig {
    pub fn default_index(&self) -> Option<usize> {
        self.default
            .as_ref()
            .and_then(|name| self.items.iter().position(|action| &action.name == name))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub key: Bindings,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub availability: Option<ActionAvailability>,
    #[serde(default)]
    pub when: Vec<ActionCondition>,
    #[serde(default)]
    pub on_success: OnSuccess,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionAvailability {
    pub command: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "default_availability_cache_ms")]
    pub cache_ms: u64,
    #[serde(default = "default_availability_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_availability_cache_ms() -> u64 {
    30_000
}

const fn default_availability_timeout_ms() -> u64 {
    5_000
}

impl ActionConfig {
    pub fn is_available(&self, item: &Map<String, Value>) -> bool {
        self.when.iter().all(|condition| {
            let actual = crate::item::field_value(item, &condition.field);
            match (&condition.equals, condition.is_set) {
                (Some(expected), None) => actual == Some(expected),
                (None, Some(expected)) => actual.is_some_and(|value| !value.is_null()) == expected,
                _ => false,
            }
        })
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionCondition {
    pub field: String,
    #[serde(default)]
    pub equals: Option<Value>,
    #[serde(default)]
    pub is_set: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnSuccess {
    #[default]
    Exit,
    Refresh,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FilterConfig {
    pub label: String,
    pub mode: Bindings,
    pub clear: Bindings,
    pub choices: Vec<FilterChoice>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            label: "filter".into(),
            mode: Bindings::new(["ctrl-g"]),
            clear: Bindings::new(["a"]),
            choices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FilterChoice {
    pub key: Bindings,
    pub label: String,
    pub source: String,
    pub value: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub fg: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FrecencyConfig {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for FrecencyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    pub enabled: bool,
    pub title: String,
    pub placeholder: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            title: "Vellum".into(),
            placeholder: "Search...".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    #[serde(default)]
    pub cmd: Option<String>,
    #[serde(default)]
    pub builtin: Option<BuiltinSource>,
    #[serde(default)]
    pub file: Option<PathBuf>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.0.iter().any(|binding| {
            other
                .0
                .iter()
                .any(|other| binding.code == other.code && binding.modifiers == other.modifiers)
        })
    }

    fn contains_key(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.0
            .iter()
            .any(|binding| binding.code == code && binding.modifiers == modifiers)
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
    pub spacing: u16,
    #[serde(default)]
    pub alternate_background: Option<String>,
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
    Repeated(RepeatedSegment),
    Styled(StyledSegment),
}

impl SegmentConfig {
    pub(crate) fn repeated(&self) -> Option<&RepeatedSegment> {
        if let Self::Repeated(segment) = self {
            Some(segment)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RepeatedSegment {
    pub for_each: String,
    pub token: String,
    #[serde(default)]
    pub separator: String,
    #[serde(default)]
    pub unique: bool,
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
    pub mode_foreground: String,
    pub insert_mode_background: String,
    pub normal_mode_background: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: "reset".into(),
            background: "reset".into(),
            selection_foreground: "black".into(),
            selection_background: "cyan".into(),
            border: "dark_gray".into(),
            mode_foreground: "black".into(),
            insert_mode_background: "green".into(),
            normal_mode_background: "yellow".into(),
        }
    }
}

impl Config {
    pub fn parse(input: &str) -> Result<Self> {
        Self::parse_layered(None, input)
    }

    pub fn parse_layered(global: Option<&str>, palette: &str) -> Result<Self> {
        Self::parse_layered_at(global.map(|global| (global, None)), (palette, None))
    }

    pub fn parse_layered_files(
        global: Option<(&str, &Path)>,
        palette: (&str, &Path),
    ) -> Result<Self> {
        Self::parse_layered_at(
            global.map(|(input, path)| (input, Some(path))),
            (palette.0, Some(palette.1)),
        )
    }

    fn parse_layered_at(
        global: Option<(&str, Option<&Path>)>,
        palette: (&str, Option<&Path>),
    ) -> Result<Self> {
        let (mut value, global_file_base) = match global {
            Some((global, path)) => {
                let value =
                    toml::from_str(global).context("invalid global Vellum configuration")?;
                let file_base = source_file_base(&value, path);
                (value, file_base)
            }
            None => (toml::Value::Table(Default::default()), None),
        };
        let palette_value = toml::from_str(palette.0).context("invalid Vellum palette")?;
        let palette_file_base = source_file_base(&palette_value, palette.1);
        clear_overridden_source_variant(&mut value, &palette_value);
        merge(&mut value, palette_value);
        let mut config: Self = value
            .try_into()
            .context("invalid merged Vellum configuration")?;
        config.validate()?;
        if let Some(file) = &mut config.source.file
            && file.is_relative()
            && let Some(base) = palette_file_base.or(global_file_base)
        {
            *file = base.join(&*file);
        }
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.frecency.max_entries == 0 {
            bail!("frecency.max_entries must be greater than zero");
        }
        if self
            .source
            .cmd
            .as_ref()
            .is_some_and(|cmd| cmd.trim().is_empty())
        {
            bail!("source.cmd cannot be empty");
        }
        if self
            .source
            .file
            .as_ref()
            .is_some_and(|file| file.as_os_str().is_empty())
        {
            bail!("source.file cannot be empty");
        }
        let source_kinds = usize::from(self.source.cmd.is_some())
            + usize::from(self.source.builtin.is_some())
            + usize::from(self.source.file.is_some());
        match source_kinds {
            1 => {}
            0 => bail!("source must set one of cmd, builtin, or file"),
            _ => bail!("source must set only one of cmd, builtin, or file"),
        }
        if self.item.template.is_empty() || self.item.template.iter().any(Vec::is_empty) {
            bail!("item.template must contain non-empty rows");
        }
        if self.item.value.trim().is_empty() {
            bail!("item.value cannot be empty");
        }
        if !self.filters.choices.is_empty() && self.filters.label.trim().is_empty() {
            bail!("filters.label cannot be empty when filter choices are configured");
        }
        let global_binding_conflicts = |bindings: &Bindings| {
            self.keybindings.enabled
                && [
                    &self.keybindings.down,
                    &self.keybindings.up,
                    &self.keybindings.accept,
                    &self.keybindings.cancel,
                    &self.keybindings.forward,
                    &self.keybindings.backward,
                    &self.keybindings.start,
                    &self.keybindings.end,
                    &self.keybindings.delete_word,
                ]
                .into_iter()
                .any(|global| bindings.overlaps(global))
        };
        if let Some(name) = &self.actions.default
            && !self.actions.items.iter().any(|action| &action.name == name)
        {
            bail!("actions.default references unknown action '{name}'");
        }
        if self
            .actions
            .menu
            .contains_key(KeyCode::Char('c'), KeyModifiers::CONTROL)
            || self.actions.menu.overlaps(&self.filters.mode)
        {
            bail!("actions.menu conflicts with Ctrl-C or filters.mode");
        }
        for (index, action) in self.actions.items.iter().enumerate() {
            if action.name.trim().is_empty() || action.label.trim().is_empty() {
                bail!("action name and label cannot be empty");
            }
            match (&action.command, &action.shell) {
                (Some(command), None) if !command.is_empty() && !command[0].trim().is_empty() => {}
                (None, Some(shell)) if !shell.trim().is_empty() => {}
                (Some(_), Some(_)) => {
                    bail!(
                        "action '{}' must set only one of command or shell",
                        action.name
                    )
                }
                _ => bail!(
                    "action '{}' must set a non-empty command or shell",
                    action.name
                ),
            }
            if action.cwd.as_ref().is_some_and(|cwd| cwd.trim().is_empty()) {
                bail!("action '{}' cwd cannot be empty", action.name);
            }
            if let Some(availability) = &action.availability {
                if availability.command.is_empty() || availability.command[0].trim().is_empty() {
                    bail!(
                        "action '{}' availability command cannot be empty",
                        action.name
                    );
                }
                if availability
                    .cwd
                    .as_ref()
                    .is_some_and(|cwd| cwd.trim().is_empty())
                {
                    bail!("action '{}' availability cwd cannot be empty", action.name);
                }
                if availability.cache_ms == 0 {
                    bail!(
                        "action '{}' availability cache_ms must be positive",
                        action.name
                    );
                }
                if availability.timeout_ms == 0 {
                    bail!(
                        "action '{}' availability timeout_ms must be positive",
                        action.name
                    );
                }
            }
            if action
                .key
                .contains_key(KeyCode::Char('c'), KeyModifiers::CONTROL)
            {
                bail!("action '{}' key conflicts with Ctrl-C", action.name);
            }
            if action.key.overlaps(&self.actions.menu) {
                bail!("action '{}' key conflicts with actions.menu", action.name);
            }
            if action.key.overlaps(&self.filters.mode) {
                bail!("action '{}' key conflicts with filters.mode", action.name);
            }
            if global_binding_conflicts(&action.key)
                || (self.search.enabled
                    && action
                        .key
                        .0
                        .iter()
                        .any(|binding| binding.modifiers == KeyModifiers::NONE))
            {
                bail!(
                    "action '{}' key conflicts with input or a global binding",
                    action.name
                );
            }
            if self.actions.items[..index]
                .iter()
                .any(|other| action.name == other.name || action.key.overlaps(&other.key))
            {
                bail!("action names and keys must be unique");
            }
            for condition in &action.when {
                if condition.field.trim().is_empty()
                    || condition.equals.is_some() == condition.is_set.is_some()
                {
                    bail!(
                        "action '{}' conditions require a field and exactly one of equals or is_set",
                        action.name
                    );
                }
                if condition
                    .equals
                    .as_ref()
                    .is_some_and(|value| value.is_array() || value.is_object())
                {
                    bail!("action '{}' condition equals must be a scalar", action.name);
                }
            }
        }
        let navigation_conflicts = |bindings: &Bindings| {
            self.keybindings.enabled
                && (bindings.overlaps(&self.keybindings.down)
                    || bindings.overlaps(&self.keybindings.up))
        };
        let input_conflicts = self.search.enabled
            && self
                .filters
                .mode
                .0
                .iter()
                .any(|binding| !binding.modifiers.contains(KeyModifiers::CONTROL));
        if self
            .filters
            .mode
            .contains_key(KeyCode::Char('c'), KeyModifiers::CONTROL)
        {
            bail!("filter mode binding conflicts with Ctrl-C");
        }
        if global_binding_conflicts(&self.filters.mode) || input_conflicts {
            bail!("filter mode binding conflicts with input or a global binding");
        }
        if self
            .filters
            .clear
            .contains_key(KeyCode::Esc, KeyModifiers::NONE)
            || self
                .filters
                .clear
                .contains_key(KeyCode::Char('c'), KeyModifiers::CONTROL)
            || self.filters.clear.overlaps(&self.filters.mode)
            || navigation_conflicts(&self.filters.clear)
        {
            bail!("filter clear binding conflicts with a reserved filter-mode key");
        }
        for choice in &self.filters.choices {
            if choice.key.0.is_empty() {
                bail!("filter choice key cannot be disabled");
            }
            if choice.label.trim().is_empty() || choice.source.trim().is_empty() {
                bail!("filter choice label and source cannot be empty");
            }
            if choice.key.contains_key(KeyCode::Esc, KeyModifiers::NONE)
                || choice
                    .key
                    .contains_key(KeyCode::Char('c'), KeyModifiers::CONTROL)
                || choice.key.overlaps(&self.filters.mode)
                || choice.key.overlaps(&self.filters.clear)
                || navigation_conflicts(&choice.key)
            {
                bail!("filter choice key conflicts with a reserved filter-mode key");
            }
        }
        for (index, choice) in self.filters.choices.iter().enumerate() {
            if self.filters.choices[..index]
                .iter()
                .any(|other| choice.key.overlaps(&other.key))
            {
                bail!("filter choice keys must be unique");
            }
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
        for repeated in self
            .item
            .template
            .iter()
            .flatten()
            .filter_map(SegmentConfig::repeated)
        {
            if repeated.for_each.trim().is_empty() || repeated.token.trim().is_empty() {
                bail!("repeated segment for_each and token cannot be empty");
            }
        }
        Ok(())
    }
}

fn clear_overridden_source_variant(base: &mut toml::Value, overlay: &toml::Value) {
    let Some(source) = overlay.get("source").and_then(toml::Value::as_table) else {
        return;
    };
    let Some(base_source) = base.get_mut("source").and_then(toml::Value::as_table_mut) else {
        return;
    };
    if ["cmd", "builtin", "file"]
        .into_iter()
        .any(|kind| source.contains_key(kind))
    {
        for kind in ["cmd", "builtin", "file"] {
            if !source.contains_key(kind) {
                base_source.remove(kind);
            }
        }
    }
}

fn source_file_base(value: &toml::Value, config_path: Option<&Path>) -> Option<PathBuf> {
    value
        .get("source")?
        .get("file")?
        .as_str()
        .and(config_path)
        .map(|path| path.parent().unwrap_or_else(|| Path::new(".")).to_owned())
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
        assert_eq!(config.search.title, "Vellum");
        assert_eq!(config.keybindings.accept.label(), "enter");
        assert_eq!(config.keybindings.down.0[1].label, "ctrl-n");
        assert_eq!(config.filters.mode.label(), "ctrl-g");
        assert_eq!(config.filters.clear.label(), "a");
        assert_eq!(config.filters.label, "filter");
        assert!(config.filters.choices.is_empty());
        assert!(config.input.vim);
        assert_eq!(config.input.start_mode, InputMode::Insert);
        assert_eq!(config.item.padding, 1);
        assert_eq!(config.item.spacing, 0);
        assert_eq!(config.item.alternate_background, None);
        assert_eq!(config.theme.selection_background, "cyan");
    }

    #[test]
    fn cfg_008_parses_builtin_source_and_rejects_ambiguous_source() {
        let builtin = MINIMAL.replace("cmd = \"herdr agents --json\"", "builtin = \"files\"");
        let config = Config::parse(&builtin).unwrap();
        assert_eq!(config.source.builtin, Some(BuiltinSource::Files));

        let ambiguous = MINIMAL.replace(
            "cmd = \"herdr agents --json\"",
            "cmd = \"herdr agent list\"\nbuiltin = \"herdr-agents\"",
        );
        assert!(
            Config::parse(&ambiguous)
                .unwrap_err()
                .to_string()
                .contains("only one")
        );
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
            spacing = 1
        "#;
        let palette = format!("{MINIMAL}\n[search]\nplaceholder = 'Palette'");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.search.placeholder, "Palette");
        assert_eq!(config.input.start_mode, InputMode::Normal);
        assert_eq!(config.keybindings.down.label(), "ctrl-j");
        assert_eq!(config.item.padding, 3);
        assert_eq!(config.item.spacing, 1);
    }

    #[test]
    fn cfg_009_palette_source_kind_replaces_global_source_kind() {
        let global = "[source]\ncmd = 'global command'";
        let palette = MINIMAL.replace("cmd = \"herdr agents --json\"", "builtin = \"files\"");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.source.cmd, None);
        assert_eq!(config.source.builtin, Some(BuiltinSource::Files));
    }

    #[test]
    fn src_012_explicit_and_named_palette_paths_have_stable_bases() {
        let palette = MINIMAL.replace("cmd = \"herdr agents --json\"", "file = 'data/items.json'");

        let explicit = Config::parse_layered_files(
            None,
            (&palette, Path::new("project/palettes/custom.toml")),
        )
        .unwrap();
        let named = Config::parse_layered_files(
            None,
            (
                &palette,
                Path::new("/home/me/.config/vellum/palettes/named.toml"),
            ),
        )
        .unwrap();

        assert_eq!(
            explicit.source.file.as_deref(),
            Some(Path::new("project/palettes/data/items.json"))
        );
        assert_eq!(
            named.source.file.as_deref(),
            Some(Path::new(
                "/home/me/.config/vellum/palettes/data/items.json"
            ))
        );

        let absolute_palette = palette.replace("data/items.json", "/shared/items.json");
        let absolute = Config::parse_layered_files(
            None,
            (&absolute_palette, Path::new("project/palettes/custom.toml")),
        )
        .unwrap();
        assert_eq!(
            absolute.source.file.as_deref(),
            Some(Path::new("/shared/items.json"))
        );
    }

    #[test]
    fn src_013_inherited_global_file_paths_keep_the_global_base() {
        let global = "[source]\nfile = 'data/items.yaml'\nrefresh_ms = 500";
        let palette = r#"
            [item]
            template = [["$name"]]
            value = "$id"
        "#;

        let config = Config::parse_layered_files(
            Some((global, Path::new("/home/me/.config/vellum/config.toml"))),
            (palette, Path::new("/work/palettes/custom.toml")),
        )
        .unwrap();

        assert_eq!(
            config.source.file.as_deref(),
            Some(Path::new("/home/me/.config/vellum/data/items.yaml"))
        );
        assert_eq!(config.source.refresh_ms, 500);
    }

    #[test]
    fn src_014_file_source_participates_in_source_kind_merging() {
        let global = "[source]\ncmd = 'global command'\nrefresh_ms = 500";
        let palette = MINIMAL.replace("cmd = \"herdr agents --json\"", "file = 'items.json'");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.source.cmd, None);
        assert_eq!(config.source.builtin, None);
        assert_eq!(config.source.file.as_deref(), Some(Path::new("items.json")));
        assert_eq!(config.source.refresh_ms, 500);

        for extra in ["cmd = 'also a command'", "builtin = 'files'"] {
            let ambiguous = palette.replace(
                "file = 'items.json'",
                &format!("file = 'items.json'\n{extra}"),
            );
            assert!(
                Config::parse(&ambiguous)
                    .unwrap_err()
                    .to_string()
                    .contains("only one")
            );
        }
        let empty = palette.replace("file = 'items.json'", "file = ''");
        assert!(
            Config::parse_layered_files(None, (&empty, Path::new("palette.toml")))
                .unwrap_err()
                .to_string()
                .contains("source.file cannot be empty")
        );
    }

    #[test]
    fn cfg_010_search_title_parses_and_layers() {
        let global = "[search]\ntitle = 'Global'";
        let palette = format!("{MINIMAL}\n[search]\ntitle = 'Files'");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.search.title, "Files");
        assert_eq!(Config::parse(MINIMAL).unwrap().search.title, "Vellum");
    }

    #[test]
    fn cfg_011_filter_configuration_parses_and_layers() {
        let global = "[filters]\nlabel = 'state'\nmode = 'ctrl-x'\nclear = 'z'";
        let palette = format!(
            "{MINIMAL}\n{}",
            r#"
            [[filters.choices]]
            key = "w"
            label = "working"
            source = "agent_status"
            value = "working"
            icon = "●"
            fg = "blue"
            "#
        );

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert_eq!(config.filters.mode.label(), "ctrl-x");
        assert_eq!(config.filters.clear.label(), "z");
        assert_eq!(config.filters.label, "state");
        assert_eq!(config.filters.choices[0].key.label(), "w");
        assert_eq!(config.filters.choices[0].label, "working");
        assert_eq!(config.filters.choices[0].source, "agent_status");
        assert_eq!(config.filters.choices[0].value, "working");
        assert_eq!(config.filters.choices[0].icon, "●");
        assert_eq!(config.filters.choices[0].fg.as_deref(), Some("blue"));

        for key in ["ctrl-c", "ctrl-n", "esc", "ctrl-x"] {
            let invalid = palette.replace("key = \"w\"", &format!("key = \"{key}\""));
            assert!(
                Config::parse_layered(Some(global), &invalid)
                    .unwrap_err()
                    .to_string()
                    .contains("conflicts")
            );
        }

        for key in ["ctrl-c", "ctrl-f", "enter", "esc", "g"] {
            let invalid_mode = global.replace("ctrl-x", key);
            assert!(Config::parse_layered(Some(&invalid_mode), &palette).is_err());
        }

        let aliases = palette.replace(
            "value = \"working\"",
            "value = \"working\"\n\n[[filters.choices]]\nkey = \"escape\"\nlabel = \"one\"\nsource = \"state\"\nvalue = \"one\"\n\n[[filters.choices]]\nkey = \"esc\"\nlabel = \"two\"\nsource = \"state\"\nvalue = \"two\"",
        );
        assert!(Config::parse_layered(None, &aliases).is_err());
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

    #[test]
    fn frc_005_frecency_settings_parse_and_layer() {
        let global = "[frecency]\nenabled = false\nmax_entries = 25";
        let palette = format!("{MINIMAL}\n[frecency]\nenabled = true");

        let config = Config::parse_layered(Some(global), &palette).unwrap();

        assert!(config.frecency.enabled);
        assert_eq!(config.frecency.max_entries, 25);
        assert_eq!(Config::parse(MINIMAL).unwrap().frecency.max_entries, 1_000);
    }

    #[test]
    fn act_001_action_configuration_parses_and_validates() {
        let config = Config::parse(&format!(
            "{MINIMAL}\n{}",
            r#"
            [actions]
            default = "focus"
            menu = "ctrl-o"

            [[actions.items]]
            name = "focus"
            label = "Focus"
            icon = "→"
            description = "Focus the selected workspace"
            key = "ctrl-r"
            command = ["herdr", "workspace", "focus", "$id"]
            cwd = "$checkout_path"
            availability = { command = ["test", "-d", "$checkout_path"], cwd = "$checkout_path", cache_ms = 5000, timeout_ms = 2000 }
            when = [{ field = "focused", equals = false }]

            [[actions.items]]
            name = "remove"
            label = "Remove"
            shell = "hwt remove --workspace '$id'"
            on_success = "refresh"
            "#
        ))
        .unwrap();

        assert_eq!(config.actions.default_index(), Some(0));
        assert_eq!(config.actions.menu.label(), "ctrl-o");
        assert_eq!(config.actions.items[0].icon, "→");
        assert_eq!(
            config.actions.items[0].description,
            "Focus the selected workspace"
        );
        assert_eq!(config.actions.items[0].command.as_ref().unwrap()[3], "$id");
        assert_eq!(
            config.actions.items[0].cwd.as_deref(),
            Some("$checkout_path")
        );
        let availability = config.actions.items[0].availability.as_ref().unwrap();
        assert_eq!(availability.command[2], "$checkout_path");
        assert_eq!(availability.cwd.as_deref(), Some("$checkout_path"));
        assert_eq!(availability.cache_ms, 5_000);
        assert_eq!(availability.timeout_ms, 2_000);
        assert_eq!(config.actions.items[0].when[0].equals, Some(false.into()));
        assert_eq!(config.actions.items[1].on_success, OnSuccess::Refresh);
        let available = serde_json::json!({"focused": false})
            .as_object()
            .unwrap()
            .clone();
        let unavailable = serde_json::json!({"focused": true})
            .as_object()
            .unwrap()
            .clone();
        assert!(config.actions.items[0].is_available(&available));
        assert!(!config.actions.items[0].is_available(&unavailable));

        let unknown = format!("{MINIMAL}\n[actions]\ndefault = 'missing'");
        assert!(
            Config::parse(&unknown)
                .unwrap_err()
                .to_string()
                .contains("unknown action")
        );

        let ambiguous = format!(
            "{MINIMAL}\n[[actions.items]]\nname='x'\nlabel='X'\ncommand=['true']\nshell='true'"
        );
        assert!(
            Config::parse(&ambiguous)
                .unwrap_err()
                .to_string()
                .contains("only one")
        );
    }
}
