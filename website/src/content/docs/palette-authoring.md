---
title: Palette authoring
description: Design a Vellum palette from structured input through output and actions.
---

A useful palette answers four questions:

1. Where do records come from?
2. Which fields help someone choose?
3. What value should acceptance print?
4. Which operations belong beside selection?

## Start with stable records

Every source entry must be an object. Give records a stable value field such as an ID, pane ID, or path. Stable values let Vellum preserve selection across refreshes and scope [frecency](../frecency/) history usefully.

```toml
[source]
cmd = "my-tool list --json"
refresh_ms = 1000

[item]
value = "$id"
template = [["$name"], ["$description"]]
```

## Add behavior deliberately

- Mark decorative or rapidly changing template fields `searchable = false`.
- Use exact [filters](../filters-input/) for categories and fuzzy search for free-form recall.
- Prefer direct argv [actions](../actions/) over shell actions.
- Put reusable input, theme, and frecency policy in the global config.
- Use `source.refresh_ms = 0` for static sources.

## Test a palette

Run a repository example without installing it:

```sh
cargo run --release -- examples/demo.toml
```

Configuration and source failures leave a diagnostic on stderr. Vellum does not yet have a headless `check` command, so open new palettes interactively and exercise refresh, every filter, and every action condition.
