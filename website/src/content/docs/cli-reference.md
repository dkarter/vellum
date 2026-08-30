---
title: CLI reference
description: Vellum commands, path resolution, output behavior, and environment variables.
---

## Synopsis

```text
vellum [PALETTE]
vellum palettes sync [--overwrite]
vellum -h | --help
vellum -V | --version
```

## `vellum [PALETTE]`

Open an interactive palette. With no argument, `PALETTE` is `default`. A single path component without an extension resolves as `<config-root>/palettes/<name>.toml`. Explicit paths and arguments with extensions are used as provided.

```sh
vellum
vellum herdr-agents
vellum ./examples/demo.toml
```

The config root is `$XDG_CONFIG_HOME/vellum`, then `$HOME/.config/vellum`. Optional global defaults load from `<config-root>/config.toml`.

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
