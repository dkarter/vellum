---
title: Frecency
description: Rank frequently and recently selected Vellum items.
---

Frecency is enabled by default. Items selected frequently and recently are hoisted above unseen entries, with exact last-use time breaking equal scores.

History is scoped by palette. Recent agents in `herdr-agents` do not affect file or workspace ordering.

```toml
[frecency]
enabled = true
max_entries = 1000
```

The bounded SQLite database lives at `$XDG_DATA_HOME/vellum/frecency.sqlite3`, falling back to `~/.local/share/vellum/frecency.sqlite3`. Set `VELLUM_DATA` to an absolute, nonempty directory to override the complete data directory.

Disable frecency globally or per palette when source order carries meaning:

```toml
[frecency]
enabled = false
```
