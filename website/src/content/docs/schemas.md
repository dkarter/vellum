---
title: Schemas
description: Add Vellum configuration validation and completion to an editor.
---

Vellum ships one JSON Schema for palettes and global defaults:

- [`schemas/vellum.schema.json`](https://github.com/dkarter/vellum/blob/main/schemas/vellum.schema.json) contains all configuration options.

The repository's `taplo.toml` associates the schema with official palettes and examples. For a configuration file elsewhere, add a schema directive as its first line:

```toml
#:schema https://raw.githubusercontent.com/dkarter/vellum/refs/heads/main/schemas/vellum.schema.json
```

The schema validates structure and many constraints. Runtime validation also checks layered source exclusivity, unique actions and keys, binding conflicts, filter conflicts, scalar conditions, and animation requirements.
