---
title: Filters and input
description: Configure fuzzy input, Vim editing, keybindings, and exact-match filters.
---

## Input editing

Readline-style bindings are enabled by default: Ctrl-N/Ctrl-P browse results, Ctrl-D/Ctrl-U move by a visible page, Ctrl-F/Ctrl-B move the cursor, Ctrl-A/Ctrl-E jump to either end, and Ctrl-W deletes the previous word. Every configurable binding accepts one key, a list of keys, or `false`.

```toml
[keybindings]
enabled = true
down = ["down", "ctrl-n"]
up = ["up", "ctrl-p"]
page_down = "ctrl-d"
page_up = "ctrl-u"
accept = "enter"
cancel = "esc"
```

A configured action menu takes precedence over an overlapping input binding. Move `actions.menu` away from Ctrl-A if you want to keep start-of-input on that key.

## Vim mode

Vim input is enabled and starts in insert mode by default:

```toml
[input]
vim = true
start_mode = "insert"
```

Escape changes insert to normal; Escape again closes Vellum. Normal mode supports `h`, `l`, `b`, `w`, `0`, `$`, `x`, `i`, `a`, `I`, `A`, `j`, and `k`. Ctrl-C always cancels. The footer badge and terminal cursor show the current mode.

## Exact-match filters

Filters combine with fuzzy search and frecency ranking:

```toml
[filters]
label = "status"
mode = "ctrl-g"
clear = "a"

[[filters.choices]]
key = "w"
label = "working"
source = "state"
value = "in_progress"
icon = "●"
fg = "yellow"
```

Press Ctrl-G, then a choice key. Press the active choice again or the clear key to show all items. Another choice replaces the active filter. Escape closes filter mode without changing Vim mode or cancelling. Filter sources support dot paths such as `metadata.state`.

Choice keys cannot collide with available editing commands while filter mode is open. List navigation remains active.
