# Vellum

Vellum is a fast, customizable menu for terminal multiplexers. Use it as a
command palette, session switcher, agent picker, or fuzzy file finder. It is
inspired by [television](https://github.com/alexpasmantier/television), with a
focus on fully customizable multi-line items and live data.

## Status

Vellum is an early implementation. It supports command-backed JSON sources,
fuzzy search, custom multi-line templates, conditional and animated tokens,
themes, keybindings, and periodic live refresh. Herdr is the first intended
integration; output is deliberately generic so other multiplexers can consume
it too.

## Install

Rust 1.88 or newer is required.

```sh
cargo install --path .
```

## Run

Pass a config explicitly:

```sh
cargo run -- examples/demo.toml
```

With no argument, Vellum loads
`$XDG_CONFIG_HOME/vellum/config.toml`, falling back to
`~/.config/vellum/config.toml`.

Vellum writes the selected item's configured value to stdout. Cancellation
writes nothing. This makes shell integration straightforward:

```sh
agent_id="$(vellum agents.toml)" && herdr focus "$agent_id"
```

## Source Format

`source.cmd` runs through `sh -c`. It must print either a JSON array:

```json
[{"id":"agent-1","name":"OpenCode"}]
```

or newline-delimited JSON objects:

```json
{"id":"agent-1","name":"OpenCode"}
{"id":"agent-2","name":"Claude"}
```

Set `source.refresh_ms` to periodically rerun the command. `0`, the default,
disables refresh.

## Configuration

```toml
[search]
enabled = true
placeholder = "Find an agent..."

[source]
cmd = "herdr agents --json"
refresh_ms = 1000

[keybindings]
down = "ctrl-n"
up = "ctrl-p"
accept = "enter"
cancel = "esc"

[item]
border = false
value = "$agent_id"
template = [
  [
    { token = "$agent_name", bold = true },
    "  ",
    { token = "$workspace", bold = true },
    { token = "$state_icon", searchable = false, align = "right" },
  ],
  [
    { token = "$terminal_title", fg = "#c0caf5" },
    { token = "$pr", fg = "#bb9af7", align = "right" },
  ],
]

[[item.tokens]]
name = "state_icon"
source = "state"
when = ["in_progress"]
fg = "yellow"
animation_fps = 3
animation_frames = ["◐", "◓", "◑", "◒"]

[[item.tokens]]
name = "state_icon"
source = "state"
when = ["idle"]
text = "●"
fg = "green"

[theme]
foreground = "#c0caf5"
background = "#1a1b26"
selection_foreground = "#1a1b26"
selection_background = "#7aa2f7"
border = "#565f89"
```

Template strings beginning with `$` read source fields. Dot paths such as
`$pull_request.state` access nested objects. Other strings are literals.
Segments aligned `right` share the remaining row space. All segments are fuzzy
searchable by default; set `searchable = false` for decorative or volatile
content. Item borders are disabled by default to fit cleanly inside multiplexer
popups and panes; set `item.border = true` when Vellum provides the outer chrome.

Token definitions derive a display token from another source field. Definitions
are checked in order, and `when` matches one or more exact source values. A token
can provide fixed `text` or animated frames. Segment styles override token
styles.

Colors accept Ratatui names such as `cyan`, `dark_gray`, and `reset`, or RGB hex
values such as `#7aa2f7`. A color beginning with `$` reads its value from the
source item, allowing live state-dependent colors.
