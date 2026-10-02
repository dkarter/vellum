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
builtin = "herdr-workspaces" # herdr-agents, files, or themes
refresh_ms = 1000
```

`herdr-workspaces` and `herdr-agents` consume `herdr api snapshot`. `files` invokes `fd --type f --color never --print0`. `themes` provides Vellum's built-in theme variants for the theme browser. Built-ins normalize records in process, avoiding fragile shell transformation pipelines.

## Opt-in remote pages

Command sources and the `github-prs` built-in can enable cursor pagination and optional disk caching:

```toml
[source]
cmd = "my-remote-source"

[source.remote]
page_size = 30       # 1–100
cache_ttl_ms = 300000 # five minutes; 0 disables disk caching
# probe_filters = true  # optional background default-category availability
# search_debounce_ms = 300 # optional server search after a typing pause
```

A remote command receives `VELLUM_FILTER` (active choice value, or empty for all), `VELLUM_QUERY` (committed server query, or empty), `VELLUM_CURSOR` (empty on the first page), and `VELLUM_PAGE_SIZE`. These are environment values, not shell substitutions into the command. It must return one JSON object:

```json
{"items": [{"id": "one", "category": "open"}], "next_cursor": "opaque-cursor"}
```

Omit `next_cursor` or return `null` when exhausted. Empty or unchanged cursors are rejected. Items must still contain the fields used by the active exact-match filter. Changing a filter cancels its old request, loads that filter's cache or first page, and ignores obsolete results.

Fuzzy matching is always immediate over loaded items. Opt in to server search with `search_debounce_ms`: after the typing pause, the command receives the latest `VELLUM_QUERY`. Its matches augment loaded items while keeping the fuzzy query and selected identity. New edits cancel obsolete requests, and caches/cursors belong to both the filter and committed server query. Clearing the query restores default pagination immediately. Without this option, query edits never fetch from the source.

With `probe_filters = true`, Vellum checks unsearched categories in the background, paging until it finds an item or reaches exhaustion. It caches availability for `cache_ttl_ms`. Empty or unknown choices are dimmed and skipped by nonempty cycling, but direct shortcuts still select them. Producers may instead include `filter_availability` in their page response: an object mapping filter choice values (and `""` for all) to booleans. Availability describes default categories, not matches for the current search.

Downward navigation at the end loads another page asynchronously. Pages append unique `item.value` identities while preserving selection. Requests show footer loading text; errors retain existing items and leave the interface usable. Downward navigation retries a failed first or next page.

Fresh cache entries skip the initial fetch. Expired entries render immediately and refresh in the background. The freshness timestamp belongs to the first page, so fetching more pages does not extend the TTL. Cache I/O failures are nonfatal. `source.refresh_ms` remains optional and refreshes the active filter from its first page.

Cache writes and eviction run in the background. Snapshots are capped at eight MiB, with at most 100 files retained after pruning. On Unix, cache files use owner-only read/write permissions. Oversized or unreadable snapshots are treated as cache misses.

Palettes without `source.remote` keep their existing source behavior. File and stdin sources cannot enable remote pages.

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
printf '%s\n' '{"id":"one","name":"First"}' | vlm custom.toml
```

Standard input is consumed once before the interface starts. It cannot be combined with `source.refresh_ms` or actions using `on_success = "refresh"`.

CLI source options override the palette's configured source for one run:

```sh
# Plain lines need no palette or mapping.
fd --type f | vlm --stdin

# Copy dotted JSON fields to names expected by the palette.
producer | vlm custom --stdin --field title=details.name --field value=id

# Use the installed jq executable for arbitrary JSON transformations.
producer | vlm custom --jq '.[] | {id, name}'
```

`--field TARGET=SOURCE` is repeatable and preserves the original fields. `SOURCE` may be a dotted object path. `--jq` runs `jq -c FILTER`, so `jq` must be available on `PATH`.

Without an explicit palette, `--stdin` uses a minimal finder that displays and returns each plain line. Automatic `--stdin` input also accepts JSON arrays and NDJSON. Plain-line items expose the same text as `value`, `name`, and `path`, so they work with simple custom palettes too.

## Refresh

`source.refresh_ms` periodically reruns the source. `0`, the default, disables refresh. Refresh is asynchronous and preserves the search query; selection follows the same `item.value` when it still exists. Source commands superseded by a newer refresh are not yet cancelled.
