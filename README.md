# Vellum

Vellum is a fast, customizable menu for integrating with your favorite terminal
multiplexer. You can use it to create a command palette, session switcher, or
fuzzy file finder.

Vellum is heavily inspired by [television](https://github.com/alexpasmantier/television) but designed to have a more customizable UI that fits and looks better with your setup.

# Terminal multiplexer support

Vellum integrates out of the box with herdr. Support for other terminal
multiplexers will be added in the near future.

# Designing

You can design multi line items in your list and align them however you want.
You can then customize which of the fields would match when fuzzy finding
through items, and what the value would be for a selected item - so what the
user sees can be different than the item sent to another program.

Items can also pull live information, show progress and continue to update once displayed

Example items:

```
 OpenCode | Dotfiles                    In progress
 Implementing feature X             PR #1234 [draft]
```

Defining the item display

agents.toml

```toml
[search]
enabled = true # default

[source]
# this is where we get the tokens from - this can be a command that returns json
# we should see what TV does here and copy that idea
cmd = "... some herdr command here ..."

[keybindings]
# we should see what TV does here and copy that idea

[item]
tokens = [
    # ... need to define other tokens
    {
        token = "$state_icon",
        when = 'in_progress',
        fg = "$agent_state_color"
        animation_fps = 3,
        animation_frames = ['', '', '', '', '', '']
    },
    {
        token = "$state_icon",
        when = ['idle'],
        text = ""
        fg = "$agent_state_color"
    }
]

template = [
  [
    "$agent_icon",
    { token = "$agent_name", bold = true },
    { token = "$workspace", bold = true },
    { token = "$agent_state_icon", fg = "$agent_state_color" },
    { token = "$agent_state_label", fg = "$agent_state_color" },
  ],
  [
    { token = "$state_icon",  searchable = false},
    { token = "$terminal_title_stripped", fg = "#c0caf5" },
    { token = "$pr", fg = "#bb9af7", align = "right" },
    { token = "$pr_state", fg = "#bb9af7", align = "right" },
  ],
]

value = "$agent_id"

```

# Theming

Out of the box Vellum uses to your herdr theme colors. But you can create custom
themes and adjust item template foreground and background and background color.
