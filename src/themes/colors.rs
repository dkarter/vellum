//! Standard ANSI palette colors, separate from UI roles like selection and border.
use crate::config::{Theme, ThemeColors};

pub(super) const NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];
const TERMINAL: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "gray",
    "dark_gray",
    "light_red",
    "light_green",
    "light_yellow",
    "light_blue",
    "light_magenta",
    "light_cyan",
    "white",
];

fn palette(id: &str) -> [&'static str; 16] {
    match id {
        "tokyo-day" => [
            "#e9e9ed", "#f52a65", "#587539", "#8c6c3e", "#2e7de9", "#9854f1", "#007197", "#6172b0",
            "#a1a6c5", "#f52a65", "#587539", "#8c6c3e", "#2e7de9", "#9854f1", "#007197", "#3760bf",
        ],
        "tokyo-moon" => [
            "#1e2030", "#ff757f", "#c3e88d", "#ffc777", "#82aaff", "#c099ff", "#86e1fc", "#828bb8",
            "#444a73", "#ff757f", "#c3e88d", "#ffc777", "#82aaff", "#c099ff", "#86e1fc", "#c8d3f5",
        ],
        "catppuccin-latte" => [
            "#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#8839ef", "#179299", "#acb0be",
            "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#8839ef", "#179299", "#bcc0cc",
        ],
        "catppuccin-frappe" => [
            "#51576d", "#e78284", "#a6d189", "#e5c890", "#8caaee", "#ca9ee6", "#81c8be", "#a5adce",
            "#626880", "#e78284", "#a6d189", "#e5c890", "#8caaee", "#ca9ee6", "#81c8be", "#b5bfe2",
        ],
        "catppuccin-macchiato" => [
            "#494d64", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#c6a0f6", "#8bd5ca", "#a5adcb",
            "#5b6078", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#c6a0f6", "#8bd5ca", "#b8c0e0",
        ],
        "catppuccin-mocha" => [
            "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#cba6f7", "#94e2d5", "#bac2de",
            "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#cba6f7", "#94e2d5", "#a6adc8",
        ],
        "solarized-dark" | "solarized-light" => [
            "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5",
            "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4", "#93a1a1", "#fdf6e3",
        ],
        "one-dark" => [
            "#282c34", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
            "#5c6370", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#ffffff",
        ],
        "everforest-dark" => [
            "#2d353b", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa",
            "#7a8478", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa",
        ],
        "everforest-light" => [
            "#5c6a72", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#dfddc8",
            "#939f91", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#fdf6e3",
        ],
        "nord" => [
            "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0", "#e5e9f0",
            "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#8fbcbb", "#eceff4",
        ],
        "rose-pine" | "rose-pine-moon" => [
            "#26233a", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4",
            "#6e6a86", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4",
        ],
        "rose-pine-dawn" => [
            "#f2e9e1", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9", "#d7827e", "#575279",
            "#9893a5", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9", "#d7827e", "#575279",
        ],
        "kanagawa-wave" => [
            "#090618", "#c34043", "#76946a", "#c0a36e", "#7e9cd8", "#957fb8", "#6a9589", "#c8c093",
            "#727169", "#e82424", "#98bb6c", "#e6c384", "#7fb4ca", "#938aa9", "#7aa89f", "#dcd7ba",
        ],
        "kanagawa-dragon" => [
            "#0d0c0c", "#c4746e", "#8a9a7b", "#c4b28a", "#8ba4b0", "#a292a3", "#8ea4a2", "#c8c093",
            "#a6a69c", "#e46876", "#87a987", "#e6c384", "#7fb4ca", "#938aa9", "#7aa89f", "#c5c9c5",
        ],
        "kanagawa-lotus" => [
            "#f2ecbc", "#c84053", "#6f894e", "#77713f", "#4d699b", "#b35b79", "#597b75", "#545464",
            "#716e61", "#d7474b", "#6e915f", "#836f4a", "#6693bf", "#624c83", "#5e857a", "#43436c",
        ],
        id if id.starts_with("dracula") => [
            "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
            "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
        ],
        id if id.starts_with("gruvbox-light") => [
            "#fbf1c7", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#7c6f64",
            "#928374", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58", "#3c3836",
        ],
        id if id.starts_with("gruvbox-dark") => [
            "#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#a89984",
            "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c", "#ebdbb2",
        ],
        _ => [
            "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
            "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5",
        ],
    }
}

pub(super) fn preset_colors(id: &str) -> ThemeColors {
    let colors = palette(id).map(|color| Some(color.to_owned()));
    let [
        black,
        red,
        green,
        yellow,
        blue,
        magenta,
        cyan,
        white,
        bright_black,
        bright_red,
        bright_green,
        bright_yellow,
        bright_blue,
        bright_magenta,
        bright_cyan,
        bright_white,
    ] = colors;
    ThemeColors {
        black,
        red,
        green,
        yellow,
        blue,
        magenta,
        cyan,
        white,
        bright_black,
        bright_red,
        bright_green,
        bright_yellow,
        bright_blue,
        bright_magenta,
        bright_cyan,
        bright_white,
    }
}

/// Old saved presets infer the full palette; explicit per-color overrides win.
pub fn resolve_color<'a>(theme: &'a Theme, color: &'a str) -> &'a str {
    if let Some(name) = color.strip_prefix("theme.colors.") {
        if let Some(value) = theme.colors.get(name) {
            return value;
        }
        if let Some(index) = NAMES.iter().position(|candidate| *candidate == name) {
            return super::PRESETS
                .iter()
                .find(|preset| {
                    preset.background == theme.background && preset.foreground == theme.foreground
                })
                .map_or(TERMINAL[index], |preset| palette(preset.id)[index]);
        }
    }
    match color {
        "theme.foreground" => &theme.foreground,
        "theme.background" => &theme.background,
        "theme.accent" => &theme.selection_background,
        "theme.muted" | "theme.border" => &theme.border,
        _ => color,
    }
}
