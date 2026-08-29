<p align="center">
  <img width="200" src="https://private-user-images.githubusercontent.com/551858/626896706-90c255f3-d30b-4121-9cb8-a527b078ed49.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3ODUxMjI4MDIsIm5iZiI6MTc4NTEyMjUwMiwicGF0aCI6Ii81NTE4NTgvNjI2ODk2NzA2LTkwYzI1NWYzLWQzMGItNDEyMS05Y2I4LWE1MjdiMDc4ZWQ0OS5wbmc_WC1BbXotQWxnb3JpdGhtPUFXUzQtSE1BQy1TSEEyNTYmWC1BbXotQ3JlZGVudGlhbD1BS0lBVkNPRFlMU0E1M1BRSzRaQSUyRjIwMjYwNzI3JTJGdXMtZWFzdC0xJTJGczMlMkZhd3M0X3JlcXVlc3QmWC1BbXotRGF0ZT0yMDI2MDcyN1QwMzIxNDJaJlgtQW16LUV4cGlyZXM9MzAwJlgtQW16LVNpZ25hdHVyZT1jN2JjNTliNmFhNzEzZjFlMDc2NTUwOWFmNDE4NjUyNmY4N2FkM2JmY2U2MDgwMzE2OGE3ZjE1MzcwYTkwYzM1JlgtQW16LVNpZ25lZEhlYWRlcnM9aG9zdCZyZXNwb25zZS1jb250ZW50LXR5cGU9aW1hZ2UlMkZwbmcifQ.YDsE_Qsg4DPYWC98nwNyUIaLsmVSEp9jsoy4gn67EUA" />
  <br />
  <strong><em>Vellum</em></strong>
</p>

Vellum is a fast, customizable menu for terminal multiplexers. Use it as a
command palette, session switcher, agent picker, or fuzzy file finder. It is
inspired by [television](https://github.com/alexpasmantier/television), with a
focus on fully customizable multi-line items and live data.

## Status

Vellum is an early implementation. It supports command-backed JSON sources,
fuzzy search, custom multi-line templates, conditional and animated tokens,
themes, keybindings, generic selection actions, and periodic live refresh.
Herdr is the first intended integration, but sources, actions, and output are
deliberately generic.

## Install

Install a prebuilt release with mise:

```sh
mise use --global github:dkarter/vellum
```

Or build from source:

Rust 1.88 or newer is required.

```sh
cargo install --path .
```

## Run

Pass a palette path explicitly:

```sh
cargo run --release -- examples/demo.toml
```

Or store named palettes under `$XDG_CONFIG_HOME/vellum/palettes` and invoke one
by name. With no argument, Vellum opens the palette named `default`.

```sh
vellum agents # loads $XDG_CONFIG_HOME/vellum/palettes/agents.toml
```

## Official Palettes

Install Vellum's bundled palettes into the user configuration directory:

```sh
vellum palettes sync
```

The command uses `$XDG_CONFIG_HOME/vellum/palettes`, or
`$HOME/.config/vellum/palettes` when `XDG_CONFIG_HOME` is unset. Existing files
are reported and left untouched so local edits are safe. To deliberately update
all official files to the bundled versions, use:

```sh
vellum palettes sync --overwrite
```

The library includes these palettes:

| Palette            | Dependency           | Enter behavior       |
| ------------------ | -------------------- | -------------------- |
| `herdr-workspaces` | `herdr`, `hwt`       | Focus workspace      |
| `herdr-agents`     | `herdr`              | Output agent pane ID |
| `files`            | `fd` and a Nerd Font | Output file path     |

The Herdr palettes use the installed CLI's `herdr api snapshot` JSON and refresh
live agent state. The file palette runs `fd` directly and applies a compact
filetype icon map adapted from Snacks.nvim's `nvim-web-devicons` fallback.

The workspace palette focuses on Enter. Ctrl-A opens its action menu, where the
selected HWT worktree can be removed and the choices refreshed without leaving
Vellum:

```sh
vellum herdr-workspaces
```

Output-only palettes can be composed with other commands. Quote the selected
value:

```sh
pane_id="$(vellum herdr-agents)" && herdr agent focus "$pane_id"
file="$(vellum files)" && "${EDITOR:-vi}" -- "$file"
```

Optional global defaults live at `$XDG_CONFIG_HOME/vellum/config.toml`. Vellum
recursively merges the selected palette over those defaults. A palette can
therefore override one setting without repeating the rest of a global section.

For output-only selection, Vellum writes the selected item's configured value
and one newline to stdout. The interactive interface, cursor controls,
restoration, and diagnostics all use stderr. Cancellation and successful native
actions write nothing to stdout, so command substitution and pipelines never
receive terminal control sequences.

## Source Format

`source.cmd` runs through `sh -c`. It must print either a JSON array:

```json
[{ "id": "agent-1", "name": "OpenCode" }]
```

or newline-delimited JSON objects:

```json
{"id":"agent-1","name":"OpenCode"}
{"id":"agent-2","name":"Claude"}
```

Set `source.refresh_ms` to periodically rerun the command. `0`, the default,
disables refresh.

Official palettes use maintained in-process adapters instead of shell pipelines:

```toml
[source]
builtin = "herdr-workspaces" # or "herdr-agents" or "files"
refresh_ms = 1000
```

Set exactly one of `source.cmd` or `source.builtin` in a complete palette.

## Configuration

```toml
[search]
enabled = true
title = "Agents"
placeholder = "Find an agent..."

[source]
cmd = "herdr agents --json"
refresh_ms = 1000

[keybindings]
enabled = true
down = ["down", "ctrl-n"]
up = ["up", "ctrl-p"]
accept = "enter"
cancel = "esc"
forward = "ctrl-f"
backward = "ctrl-b"
start = "ctrl-a"
end = "ctrl-e"
delete_word = "ctrl-w"

[filters]
label = "state"
mode = "ctrl-g"
clear = "a"

[[filters.choices]]
key = "w"
label = "working"
source = "state"
value = "in_progress"
icon = "●"
fg = "yellow"

[[filters.choices]]
key = "i"
label = "idle"
source = "state"
value = "idle"
icon = "●"
fg = "green"

[input]
vim = true
start_mode = "insert"

[frecency]
enabled = true
max_entries = 1000

[actions]
default = "open"
menu = "ctrl-a"

[[actions.items]]
name = "open"
label = "Open item"
command = ["my-tool", "open", "$agent_id"]
on_success = "exit"

[[actions.items]]
name = "remove"
label = "Remove item"
key = "ctrl-r"
command = ["my-tool", "remove", "--id", "$agent_id"]
when = [{ field = "removable", equals = true }]
on_success = "refresh"

[item]
border = false
padding = 1
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
mode_foreground = "#1a1b26"
insert_mode_background = "#9ece6a"
normal_mode_background = "#e0af68"
```

Template strings beginning with `$` read source fields. Dot paths such as
`$pull_request.state` access nested objects. Other strings are literals.
Segments aligned `right` share the remaining row space. All segments are fuzzy
searchable by default; set `searchable = false` for decorative or volatile
content. Item borders are disabled by default to fit cleanly inside multiplexer
popups and panes; set `item.border = true` when Vellum provides the outer chrome.
Horizontal item padding defaults to `1`, aligning item text with the content
inside the search border. Set `item.padding` to any non-negative cell count.
Set `search.title` to label the input for a specific palette.

Token definitions derive a display token from another source field. Definitions
are checked in order, and `when` matches one or more exact source values. A token
can provide fixed `text` or animated frames. Segment styles override token
styles.

Colors accept Ratatui names such as `cyan`, `dark_gray`, and `reset`, or RGB hex
values such as `#7aa2f7`. A color beginning with `$` reads its value from the
source item, allowing live state-dependent colors.

## Actions

Actions are generic palette commands. Each action has a unique `name`, display
`label`, optional `icon` and `description`, one command form, and an `on_success`
policy. Labels, descriptions, and action names are fuzzy-searchable in the
quick-action menu. `command` is an argv array executed directly, never through a
shell. An argument consisting entirely of a field expression such as `$id` or
`$metadata.id` is replaced with that selected item field as one literal
argument. Strings with spaces or shell metacharacters therefore remain one
argument and cannot become shell syntax. Missing, null, array, and object fields
fail safely and leave Vellum open.

Optional `when` conditions keep an action out of direct dispatch and the menu
unless every condition matches the selected item. Use `equals` for an exact
scalar comparison, or `is_set = true` to require a present, non-null field:

```toml
when = [
  { field = "focused", equals = false },
  { field = "worktree", is_set = true },
]
```

Set `actions.default` to make the normal accept binding run that named action.
Without a default action, Enter keeps the original output-only behavior and
prints `item.value`. An action's optional `key` invokes it directly. The action
menu uses Ctrl-A by default and can be changed with `actions.menu`; it lists all
actions for the current item, uses the normal up/down and accept bindings, and
closes with Escape. Set `actions.menu = false` when only default or direct action
bindings should be available.

Try the bundled action playground without running any destructive commands:

```sh
cargo run --quiet -- examples/actions.toml
```

Press Ctrl-A and type part of a label or description, such as `error`, `rerun`,
or `ready`. Arrow keys and Ctrl-N/Ctrl-P navigate the filtered list. The refresh
action reruns the source, the error action demonstrates failure handling, and
the ready-only action disappears when `Beta project` is selected. Enter runs the
default harmless exit action.

`on_success = "exit"` closes Vellum without writing selection output.
`on_success = "refresh"` reruns the palette source, preserves the search query,
and keeps selection on the same `item.value` when it still exists. If an action
removed that item, Vellum selects the nearest remaining list position. Failed
commands and failed refreshes leave Vellum open and show the error in the
footer.

For commands that intentionally need shell syntax, set `shell = "..."` instead
of `command`. This explicit form runs through `sh -c` and does not interpolate
selected fields. Action processes are non-interactive: stdout is discarded,
stderr is captured for error reporting, and they do not receive the terminal.
Confirmation prompts are a future extension; this version relies on the invoked
command for any confirmation it can perform without interactive terminal input.

## Input

Readline-style bindings are enabled by default: Ctrl-N/Ctrl-P browse results,
Ctrl-F/Ctrl-B move the input cursor, Ctrl-A/Ctrl-E jump to either end, and Ctrl-W
deletes the previous word. A configured action menu takes precedence over an
overlapping input binding, so the default Ctrl-A menu replaces start-of-input in
palettes with actions. Set `actions.menu` to another key to retain Ctrl-A input
movement. Each binding accepts one key, a list of keys, or `false`. Set
`keybindings.enabled = false` to disable all configurable input bindings. Global
bindings and per-palette overrides use the same syntax.

Vim input mode is enabled by default and starts in insert mode. Escape changes
insert mode to normal mode; Escape again closes Vellum. A palette starting in
normal mode closes on its first Escape. Normal mode supports `h`, `l`, `b`, `w`,
`0`, `$`, `x`, `i`, `a`, `I`, `A`, `j`, and `k`. Set `input.vim = false` to use
only regular/readline editing. Ctrl-C always cancels, regardless of mode or
keybinding settings. The terminal cursor is a bar in insert mode and a block in
normal mode. When Vim input is enabled, a colored badge at the left of the
footer shows the active mode. Configure the badge with `theme.mode_foreground`,
`theme.insert_mode_background`, and `theme.normal_mode_background`.

Palettes can define single-select exact-match filters under `[filters]`. Press
the configured `mode` binding (`ctrl-g` by default) to enter filter mode, then
press a choice key to activate it. Press `a` (`filters.clear`) to show all items;
pressing the active choice again also clears the filter, while choosing another
replaces it. Escape or the mode binding closes filter mode without changing Vim
mode or cancelling Vellum. While filter mode is open, its choice keys cannot
collide with normal- or insert-mode commands. Filters narrow the existing fuzzy
and frecency-ranked results by exact string source values and support dot paths
such as `metadata.state`.

The active choice is shown beside the search title using its optional `icon` and
`fg`. Filter-mode help uses the configured short `label` and compact keys, such
as `status a/w/d/i/b/u`, instead of repeating every choice label. List
navigation remains available in filter mode, including Ctrl-N and Ctrl-P.

Filter choices are configured per palette and can also be supplied by global
defaults. The `herdr-agents` palette includes `w`, `d`, `i`, `b`, and `u` for
working, done, idle, blocked, and unknown agents.

## Frecency

Frecency is enabled by default. Entries selected frequently and recently are
hoisted above unseen entries, with exact last-use time breaking equal scores.
History is scoped by palette, so recent agents automatically appear first in
`herdr-agents` without affecting file or workspace ordering.

Vellum stores bounded history in SQLite at
`$XDG_DATA_HOME/vellum/frecency.sqlite3`, falling back to
`~/.local/share/vellum/frecency.sqlite3`. Set `VELLUM_DATA` to override the
complete data directory. Global or palette configuration can disable or bound history:

```toml
[frecency]
enabled = false
max_entries = 1000
```

## Schema

Editors using Taplo can use JSON Schema for validation and completion. This
repository includes `schemas/vellum.schema.json` for palettes and
`schemas/global.schema.json` for global defaults, with associations in
`taplo.toml`. Both entry points reference `schemas/config-options.schema.json`
for options supported in either file. For a palette elsewhere, add this first
line with an appropriate local path or the raw GitHub URL:

```toml
#:schema https://raw.githubusercontent.com/dkarter/vellum/refs/heads/main/schemas/vellum.schema.json
```
