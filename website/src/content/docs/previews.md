---
title: Previews
description: Show an asynchronous command preview beside your results.
---

Previews are off by default. Add a `[preview]` section to a palette (or to your global `config.toml`), then set `enabled = true`. The command starts as soon as the highlighted source item changes. Vellum keeps the previous result visible until the next one arrives, cancels superseded work, and reuses the 50 most recent previews. Output is limited to 128 KiB per command, and slow commands time out.

```toml
[preview]
enabled = true
position = "right" # left, right, top, or bottom
size = 55          # percentage of the results area, 10–90
title = "File"
command = ["bat", "--color=always", "--decorations=always", "--style=numbers", "--line-range=:300", "$path"]
border = "separator" # separator (default), full, none
scrollbar = true    # show a scrollbar when content overflows
scroll_up = "ctrl-u"
scroll_down = "ctrl-d"
timeout_ms = 2000
```

The command is an argv array, executed directly without a shell. ANSI SGR colors from commands such as `bat --color=always` are rendered in the preview. An argument that consists entirely of `$field` or `$nested.field` resolves to a scalar field on the selected source item; missing fields show an error in the pane. `cwd = "$directory"` supports the same substitution. Literal arguments remain literal. For shell features, explicitly use `command = ["sh", "-c", "...", "sh", "$path"]` and reference the selected path as `$1` inside the script.

Ctrl-U and Ctrl-D scroll the preview by half a viewport when previews are active; change `scroll_up` and `scroll_down` to other bindings if you use those keys for list paging. The mouse wheel scrolls whichever pane it is over: preview or results. The scrollbar appears only when the preview exceeds its viewport. A separator is the default chrome: it draws one line between results and preview. Use `border = "full"` for a complete box or `border = "none"` for no chrome. Set `item.box_title = "Results"` to give the results their own titled box.

Use `--preview` to enable a configured preview, `--no-preview` to hide it, or `--preview-position left|right|top|bottom` to enable it at that location for one invocation. A preview command must be configured to enable previews. On small terminals the pane disappears automatically so results remain usable. The preview uses your existing theme's foreground, background, border, and selection colors.

See the [files-with-preview example](https://github.com/dkarter/vellum/blob/main/examples/files-preview.toml) for a complete palette.

Other layouts: [framed results and preview](https://github.com/dkarter/vellum/blob/main/examples/files-preview-framed.toml) and [borderless preview](https://github.com/dkarter/vellum/blob/main/examples/files-preview-clean.toml).
