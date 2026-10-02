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
| `github-prs` | Authenticated `gh`, a repository checkout | Open PR in browser |

## GitHub pull requests

Run `vlm github-prs` in a repository checkout. It starts with **mine open** selected and the filter controls open:

- **m — mine open:** your open PRs.
- **r — needs my review:** other authors' open PRs with a direct review request for you, your latest substantive review dismissed, or unresolved review threads you commented on. An approval alone does not include a PR.
- **c — mine closed:** your merged or closed PRs.
- **v — reviewed closed:** merged or closed PRs you reviewed or left review comments on.

The list shows the PR state at the top right, with the author flush left on the second row alongside approvals and latest-commit check totals. State uses the selected theme's ANSI green, magenta, and red for open, merged, and closed; drafts use the muted border color. Authors use the theme's cyan. The preview includes reviewer decisions, individual checks and their URLs, unresolved thread counts, and the PR body. Enter opens the PR; Ctrl-A offers checkout and check-page actions. Escape leaves filter mode for search; Ctrl-G reopens the filters.

Empty default categories are dimmed and Tab skips them. Their letter shortcuts still work, and startup stays on mine open even if it is empty. Category availability is checked in the background and cached for five minutes; it is independent of the search text. Unknown categories are dimmed until their probe completes.

Typing fuzzy-filters loaded items immediately. After a 300 ms pause, Vellum also sends a literal title-text query to GitHub, augmenting local matches with matching PRs not loaded yet. GitHub text search is not fuzzy, so it can differ from local matching. New edits cancel obsolete requests. Search pages and caches are scoped to the category and server query; clearing the query immediately restores default pagination.

Pages contain 30 candidates, newest updates first. Navigate down to the end to fetch another page. Checklist filtering can produce an empty page even when more candidates exist; navigate down again to continue. GitHub search limits each query to 1,000 results.

The cache TTL is five minutes. Fresh cached pages render without an initial network fetch; expired pages render immediately while the footer says `refreshing source...`. Refresh also runs every five minutes while the palette stays open. Pagination preserves selection, removes duplicate values, and does not renew the cache TTL.

Caches live under `$XDG_CACHE_HOME/vellum/sources` or `~/.cache/vellum/sources`, scoped to the checkout, source settings, filter, and active GitHub account. Persistent caching is disabled for environment-token authentication or when an offline account identity is unavailable. For GitHub Enterprise, set `GH_HOST` to the repository's hostname.

## Herdr workspaces

Refreshes every second and renders aligned two-line workspace records. Ctrl-G filters working, done, idle, blocked, and unknown statuses. Enter focuses a workspace. Ctrl-A opens actions to remove eligible HWT worktrees, open repositories, and view pull requests or checks. GitHub PR actions appear only when an availability probe finds a pull request.

## Herdr agents

Refreshes every 750ms and renders three-line agent records with normalized Herdr status colors. It outputs a pane ID so another command can focus or otherwise compose with the selected agent.

## Files

Runs `fd` directly, renders a compact filetype icon and path, and outputs the selected path.

## Themes

Run `vlm palettes sync`, then `vlm themes`. Browse Tokyo Night (Night, Storm, Moon, Day), Catppuccin (Latte, Frappé, Macchiato, Mocha), Dracula, Gruvbox (Dark and Light, with Hard and Soft contrast), Solarized (Dark and Light), One Dark, Everforest (Dark and Light), Nord, Rosé Pine, and Kanagawa variants. Highlighting a theme changes Vellum's colors immediately; the adjacent preview shows sample search results and color swatches without duplicating the selector's status bar. Enter saves the choice to the global `config.toml` while retaining other sections and comments. Cancel without saving using Esc or Ctrl-C. No theme name is printed to stdout.

For an opt-in file preview, use [`examples/files-preview.toml`](https://github.com/dkarter/vellum/blob/main/examples/files-preview.toml). It uses `bat` to display the highlighted file with line numbers; install `bat` alongside `fd`, then run `vlm examples/files-preview.toml` from the repository root.

The repository also contains opt-in `examples/herdr-agents-icons.toml` and `examples/herdr-workspaces-icons.toml`. Their private-use glyphs require a compatible patched font, so they are not official defaults.
