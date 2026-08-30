---
title: Quick start
description: Sync Vellum's official palettes and build a first custom palette.
---

## Open an official palette

Install the bundled palettes into your user configuration directory:

```sh
vellum palettes sync
vellum files
```

Vellum uses `$XDG_CONFIG_HOME/vellum/palettes`, falling back to `~/.config/vellum/palettes`. Existing files are skipped. Use `vellum palettes sync --overwrite` only when you want to replace local copies with the bundled versions.

## Create a palette

Save this as `~/.config/vellum/palettes/projects.toml`:

```toml
[search]
title = "Projects"
placeholder = "Find a project..."

[source]
cmd = '''printf '%s\n' '[{"name":"Vellum","path":"~/code/vellum","language":"Rust"},{"name":"Notes","path":"~/notes","language":"Markdown"}]' '''

[item]
value = "$path"
template = [
  [{ token = "$name", bold = true }, { token = "$language", align = "right", fg = "dark_gray" }],
  [{ token = "$path", fg = "cyan" }],
]
```

Open it by name:

```sh
project="$(vellum projects)" && cd "${project/#\~/$HOME}"
```

A name without a path extension resolves beneath the user palette directory. With no argument, Vellum opens the palette named `default`. You can also pass a path directly:

```sh
vellum ./examples/demo.toml
```

## Learn the model

A palette combines a [source](../sources/), an [item template](../item-templates/), optional [filters](../filters-input/), and optional [actions](../actions/). Global defaults can provide shared keybindings and theme settings.
