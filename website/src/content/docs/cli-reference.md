---
title: CLI reference
description: Vellum commands, path resolution, output behavior, and environment variables.
---

## Synopsis

```text
vellum [PALETTE] [SOURCE OPTIONS]
vellum palettes sync [--overwrite]
vellum -h | --help
vellum -V | --version
```

## `vellum [PALETTE]`

Open an interactive palette. With no argument, `PALETTE` is `default`. A single path component without an extension resolves as `<config-root>/palettes/<name>.toml`. Explicit paths and arguments with extensions are used as provided.

```sh
vellum
vellum herdr-agents
vellum herdr-agents --select-1
vellum ./examples/demo.toml
```

The config root is `$XDG_CONFIG_HOME/vellum`, then `$HOME/.config/vellum`. Optional global defaults load from `<config-root>/config.toml`.

### Source options

| Option | Meaning |
| --- | --- |
| `--stdin` | Auto-detect plain lines, a JSON array, or NDJSON input |
| `--lines FIELD` | Wrap each nonempty input line as an object containing `FIELD` |
| `--field TARGET=SOURCE` | Copy a dotted JSON source field to a target field; repeatable |
| `--jq FILTER` | Transform input through external `jq -c` before loading it |

Use exactly one of `--stdin`, `--lines`, or `--jq`. `--field` requires one of those modes. CLI sources are one-shot: periodic refresh is disabled and actions using `on_success = "refresh"` are rejected.

When no palette is given, `--stdin` uses a minimal finder that displays and returns each plain input line. For example, `fd --type f | vellum --stdin` requires no configuration.

### Selection options

| Option | Meaning |
| --- | --- |
| `-1`, `--select-1` | Accept the initial result without opening the menu when exactly one item exists |

With zero or multiple initial items, `--select-1` opens the interactive palette normally. A sole item follows the usual accept behavior, including its configured `item.value` or default action.

### Working directory

`--cwd PATH` changes Vellum's process directory before loading the source. Source commands, availability probes, and actions inherit this directory. An action or availability probe with its own configured `cwd` continues to override the process directory. Vellum exits with an error if `PATH` cannot be resolved or entered.

For a [Herdr](https://github.com/dkarter/herdr) popup, pass the focused pane directory directly and fall back to the launch directory when the environment variable is unset:

```toml
command = "vellum hwt-urls --select-1 --cwd \"${HERDR_ACTIVE_PANE_CWD:-$PWD}\""
```

For tmux, interpolate the active pane path into the popup command:

```sh
bind-key u display-popup -E "vellum hwt-urls --select-1 --cwd '#{pane_current_path}'"
```

## `vellum palettes sync`

Copy official palettes into `<config-root>/palettes`. Existing paths are reported and skipped.

`--overwrite` replaces regular files with current bundled content. Symlink targets are refused.

## Process output

Accepted output-only selections write the configured `item.value` plus one newline to stdout. The TUI, terminal control, and diagnostics use stderr. Cancellation and successful native actions write no stdout.

## Environment

| Variable | Meaning |
| --- | --- |
| `XDG_CONFIG_HOME` | Parent directory for `vellum/config.toml` and `vellum/palettes` |
| `HOME` | Fallback configuration and data home |
| `VELLUM_DATA` | Absolute override for the Vellum data directory |
| `XDG_DATA_HOME` | Parent of the fallback `vellum/frecency.sqlite3` |

Data directory precedence is absolute nonempty `VELLUM_DATA`, `$XDG_DATA_HOME/vellum`, then `$HOME/.local/share/vellum`.
