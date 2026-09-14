---
title: Configuration
description: Understand palette files, global defaults, layering, and themes.
---

Vellum reads TOML configuration. A palette defines the data and presentation for one menu. Optional global defaults live at `$XDG_CONFIG_HOME/vellum/config.toml`, falling back to `~/.config/vellum/config.toml`.

## Working directory

Set top-level `cwd` to run palette sources and actions from a specific directory. It accepts a literal path or an exact environment variable reference using `$NAME` or `${NAME}`:

```toml
cwd = "$HERDR_ACTIVE_PANE_CWD"
```

Vellum changes directory after loading the palette and before running its source. Source commands, built-in sources, availability probes, and actions inherit that directory. An action or availability probe with its own `cwd` overrides it.

For a Herdr popup, pass the focused pane directory through the documented `HERDR_ACTIVE_PANE_CWD` environment variable and use the configuration above. A tmux popup can export its pane path directly:

```text
display-popup -E -e VELLUM_PANE_CWD='#{pane_current_path}' 'vellum hwt-urls --select-1'
```

```toml
cwd = "$VELLUM_PANE_CWD"
```

Unset environment references and paths that cannot be entered produce an error before the source runs. Relative literal paths resolve from the directory where Vellum was launched.

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
