---
title: Configuration
description: Understand palette files, global defaults, layering, and themes.
---

Vellum reads TOML configuration. A palette defines the data and presentation for one menu. Optional global defaults live at `$XDG_CONFIG_HOME/vellum/config.toml`, falling back to `~/.config/vellum/config.toml`.

## Layering

Vellum recursively merges the selected palette over global defaults. A palette can override one setting without copying an entire section. Source kinds are special: setting `cmd`, `builtin`, or `file` in a palette replaces an inherited source kind while preserving unrelated source settings such as `refresh_ms`.

After merging, exactly one source kind must be set.

## Main sections

| Section | Purpose |
| --- | --- |
| `[search]` | Input visibility, title, and placeholder |
| `[source]` | Command, built-in, or file source and refresh interval |
| `[input]` | Vim editing and starting mode |
| `[keybindings]` | Navigation, editing, acceptance, and cancellation keys |
| `[filters]` | Exact-match filter mode and choices |
| `[frecency]` | History ranking and storage bound |
| `[actions]` | Default, direct, and menu actions |
| `[item]` | Output value, layout, spacing, and tokens |
| `[theme]` | Terminal colors and mode badge colors |

## Theme example

```toml
[theme]
foreground = "#c0caf5"
background = "#1a1b26"
selection_foreground = "#1a1b26"
selection_background = "#7aa2f7"
border = "#565f89"
mode_foreground = "#1a1b26"
insert_mode_background = "#9ece6a"
normal_mode_background = "#e0af68"
```

Colors accept Ratatui names such as `cyan`, `dark_gray`, and `reset`, or RGB hex values. Some presentation colors can read a source field by using a value such as `$status_color`.

Copy-ready [Tokyo Night, Catppuccin Mocha, Dracula, Gruvbox Dark, and Nord themes](https://github.com/dkarter/vellum/tree/main/examples/themes) are available in `examples/themes/`. Use one as your global `config.toml`, or copy its `[theme]` section into an existing global or palette configuration.

See the [schema reference](../schemas/) for editor completion and the complete option inventory.
