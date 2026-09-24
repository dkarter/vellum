---
title: Official palettes
description: Use Vellum's bundled Herdr and file-finding palettes.
---

Install the bundled palettes with `vlm palettes sync`. Existing files remain untouched; `--overwrite` deliberately replaces them. Vellum refuses to overwrite symlink targets.

| Palette | Dependencies | Enter behavior |
| --- | --- | --- |
| `herdr-workspaces` | `herdr`; actions use `hwt` and `gh` | Focus workspace |
| `herdr-agents` | `herdr` | Output agent pane ID |
| `files` | `fd` and a Nerd Font | Output file path |
| `themes` | None | Save selected theme to global config |

## Herdr workspaces

Refreshes every second and renders aligned two-line workspace records. Ctrl-G filters working, done, idle, blocked, and unknown statuses. Enter focuses a workspace. Ctrl-A opens actions to remove eligible HWT worktrees, open repositories, and view pull requests or checks. GitHub PR actions appear only when an availability probe finds a pull request.

## Herdr agents

Refreshes every 750ms and renders three-line agent records with normalized Herdr status colors. It outputs a pane ID so another command can focus or otherwise compose with the selected agent.

## Files

Runs `fd` directly, renders a compact filetype icon and path, and outputs the selected path.

## Themes

Run `vlm palettes sync`, then `vlm themes`. Browse Tokyo Night (Night, Storm, Moon, Day), Catppuccin (Latte, Frappé, Macchiato, Mocha), Dracula, Gruvbox (Dark and Light, with Hard and Soft contrast), Solarized (Dark and Light), One Dark, Everforest (Dark and Light), Nord, Rosé Pine, and Kanagawa variants. Highlighting a theme changes Vellum's colors immediately; the adjacent preview shows a complete sample palette and its color swatches without blending them into the list highlight. Enter saves the choice to the global `config.toml` while retaining other sections and comments. Cancel without saving using Esc or Ctrl-C. No theme name is printed to stdout.

For an opt-in file preview, use [`examples/files-preview.toml`](https://github.com/dkarter/vellum/blob/main/examples/files-preview.toml). It uses `bat` to display the highlighted file with line numbers; install `bat` alongside `fd`, then run `vlm examples/files-preview.toml` from the repository root.

The repository also contains opt-in `examples/herdr-agents-icons.toml` and `examples/herdr-workspaces-icons.toml`. Their private-use glyphs require a compatible patched font, so they are not official defaults.
