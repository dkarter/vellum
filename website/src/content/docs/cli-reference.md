---
title: CLI reference
description: Vellum commands, path resolution, output behavior, and environment variables.
---

## Synopsis

```text
vlm [PALETTE] [SOURCE OPTIONS]
vlm palettes sync [--overwrite]
vlm --completions=SHELL
vlm -h | --help
vlm -V | --version
```

## `vlm [PALETTE]`

Open an interactive palette. With no argument, `PALETTE` is `default`. A single path component without an extension resolves as `<config-root>/palettes/<name>.toml`. Explicit paths and arguments with extensions are used as provided.

```sh
vlm
vlm herdr-agents
vlm herdr-agents --select-1
vlm ./examples/demo.toml
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

When no palette is given, `--stdin` uses a minimal finder that displays and returns each plain input line. For example, `fd --type f | vlm --stdin` requires no configuration.

### Selection options

| Option | Meaning |
| --- | --- |
| `-1`, `--select-1` | Accept the initial result without opening the menu when exactly one item exists |

With zero or multiple initial items, `--select-1` opens the interactive palette normally. A sole item follows the usual accept behavior, including its configured `item.value` or default action.

### Preview options

| Option | Meaning |
| --- | --- |
| `--preview` | Enable a configured preview for this run |
| `--no-preview` | Hide previews for this run |
| `--preview-position left\|right\|top\|bottom` | Enable the preview at this position for this run |

See [previews](../previews/) for command and theme configuration.

## `vlm palettes sync`

Copy official palettes into `<config-root>/palettes`. Existing paths are reported and skipped.

`--overwrite` replaces regular files with current bundled content. Symlink targets are refused.

## Shell completions

Generate a completion script with `vlm --completions=SHELL`, where `SHELL` is `bash`, `zsh`, `fish`, `elvish`, `nu`, or `powershell`. For example:

```sh
vlm --completions=bash > ~/.local/share/bash-completion/completions/vlm
vlm --completions=fish > ~/.config/fish/completions/vlm.fish
vlm --completions=zsh > ~/.local/share/zsh/site-functions/_vlm
```

For zsh, add `fpath+=(~/.local/share/zsh/site-functions)` before `compinit` in your shell setup. The scripts call `vlm` only when completing; palette suggestions come from TOML filenames in `<config-root>/palettes` without reading their contents.

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
