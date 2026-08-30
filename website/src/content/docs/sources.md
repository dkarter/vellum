---
title: Sources
description: Feed Vellum from shell commands, built-in adapters, or structured files.
---

Set exactly one of `source.cmd`, `source.builtin`, or `source.file` after global and palette settings merge.

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

## Refresh

`source.refresh_ms` periodically reruns the source. `0`, the default, disables refresh. Refresh is asynchronous and preserves the search query; selection follows the same `item.value` when it still exists. Source commands superseded by a newer refresh are not yet cancelled.
