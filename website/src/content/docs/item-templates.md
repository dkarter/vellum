---
title: Item templates
description: Render searchable multiline records with styled and derived tokens.
---

`item.template` is an array of rows. Each row is an array of literal strings or segment objects.

```toml
[item]
value = "$agent_id"
border = false
padding = 1
spacing = 0
template = [
  [
    { token = "$agent_name", bold = true },
    { token = "$state_icon", searchable = false, align = "right" },
  ],
  [
    { token = "$terminal_title", fg = "#c0caf5" },
    { token = "$pull_request.number", fg = "#bb9af7", align = "right" },
  ],
]
```

Strings beginning with `$` read source fields. Dot paths access nested objects. Other strings are literals. Right-aligned segments share the remaining row width. All segments are fuzzy-searchable unless `searchable = false`.

Borders are disabled by default for multiplexer popups. `padding` controls horizontal cells, `spacing` inserts blank rows, and `alternate_background` separates odd visible entries without extra height.

## Repeated segments

Use `for_each` to render every value in an array:

```toml
template = [[
  "$name",
  { for_each = "$members", token = "$member_icon", separator = " ", unique = true, searchable = false, align = "right" },
]]
```

Object elements expose their fields directly. Scalar elements use `$value`; outer fields use `$parent.field`. Empty values are omitted and separators appear only between rendered values.

## Derived and animated tokens

```toml
[[item.tokens]]
name = "state_icon"
source = "state"
when = ["in_progress"]
fg = "yellow"
animation_fps = 3
animation_frames = ["◐", "◓", "◑", "◒"]

[[item.tokens]]
name = "state_icon"
source = "state"
when = ["idle"]
text = "●"
fg = "green"
```

Definitions are checked in order. `when` matches exact source values. A definition can supply fixed text, animated frames, or preserve the original source value. Segment styles override token styles.
