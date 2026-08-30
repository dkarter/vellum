---
title: Sources
description: Feed Vellum from shell commands, built-in adapters, structured files, or standard input.
---

Set exactly one of `source.cmd`, `source.builtin`, `source.file`, or `source.stdin` after global and palette settings merge.

## Command sources

`source.cmd` runs through `sh -c`. It must print a JSON array of objects:

```json
[{ "id": "agent-1", "name": "OpenCode" }]
```

It may instead print newline-delimited JSON objects:

```json
{"id":"agent-1","name":"OpenCode"}
{"id":"agent-2","name":"Claude"}
```

Use normal shell caution because the command is explicitly shell-backed. The configured command is not interpolated with selected item fields.

## Built-in sources

```toml
[source]
builtin = "herdr-workspaces" # herdr-agents or files
refresh_ms = 1000
```

`herdr-workspaces` and `herdr-agents` consume `herdr api snapshot`. `files` invokes `fd --type f --color never --print0`. Built-ins normalize records in process, avoiding fragile shell transformation pipelines.

## File sources

```toml
[source]
file = "items.json"
```

Relative paths resolve from the configuration file that declares `source.file`, not the process working directory. This remains true when a palette inherits a file source from global configuration.

| Extension | Required shape |
| --- | --- |
| `.json` | JSON array of objects or NDJSON objects |
| `.jsonc` | JSON/NDJSON with `//`, `/* */`, or `#` comments |
| `.yaml`, `.yml` | Top-level sequence of mappings |
| `.toml` | One or more `[[items]]` tables |

## Standard input

Set `source.stdin = true` to use a JSON array or NDJSON stream piped to Vellum:

```toml
[source]
stdin = true
```

```sh
printf '%s\n' '{"id":"one","name":"First"}' | vellum custom.toml
```

Standard input is consumed once before the interface starts. It cannot be combined with `source.refresh_ms` or actions using `on_success = "refresh"`.

CLI source options override the palette's configured source for one run:

```sh
# Plain lines need no palette or mapping.
fd --type f | vellum --stdin

# Copy dotted JSON fields to names expected by the palette.
producer | vellum custom --stdin --field title=details.name --field value=id

# Use the installed jq executable for arbitrary JSON transformations.
producer | vellum custom --jq '.[] | {id, name}'
```

`--field TARGET=SOURCE` is repeatable and preserves the original fields. `SOURCE` may be a dotted object path. `--jq` runs `jq -c FILTER`, so `jq` must be available on `PATH`.

Without an explicit palette, `--stdin` uses a minimal finder that displays and returns each plain line. Automatic `--stdin` input also accepts JSON arrays and NDJSON. Plain-line items expose the same text as `value`, `name`, and `path`, so they work with simple custom palettes too.

## Refresh

`source.refresh_ms` periodically reruns the source. `0`, the default, disables refresh. Refresh is asynchronous and preserves the search query; selection follows the same `item.value` when it still exists. Source commands superseded by a newer refresh are not yet cancelled.
