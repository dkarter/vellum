---
title: Schemas
description: Add Vellum configuration validation and completion to an editor.
---

Vellum ships JSON Schemas for palettes and global defaults:

- [`schemas/vellum.schema.json`](https://github.com/dkarter/vellum/blob/main/schemas/vellum.schema.json) is the palette entry point.
- [`schemas/global.schema.json`](https://github.com/dkarter/vellum/blob/main/schemas/global.schema.json) is the global-config entry point.
- [`schemas/config-options.schema.json`](https://github.com/dkarter/vellum/blob/main/schemas/config-options.schema.json) contains options shared by both.

The repository's `taplo.toml` associates the schemas with official palettes and examples. For a palette elsewhere, add a schema directive as its first line:

```toml
#:schema https://raw.githubusercontent.com/dkarter/vellum/refs/heads/main/schemas/vellum.schema.json
```

For global defaults, use the `global.schema.json` URL instead.

The schemas validate structure and many constraints. Runtime validation also checks layered source exclusivity, unique actions and keys, binding conflicts, filter conflicts, scalar conditions, and animation requirements.
