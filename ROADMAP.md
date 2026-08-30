# Roadmap

## Action System

- [ ] Run actions asynchronously without blocking input or rendering.
- [ ] Show action progress and report completion or failure in the interface.
- [ ] Support optional action timeouts and cancellation.
- [ ] Add in-process confirmation prompts for destructive actions.
- [ ] Safely interpolate selected scalar fields into confirmation messages.
- [ ] Add `on_success = "stay"` for actions that should not exit or refresh.

## Preview

- [ ] Add a responsive preview pane for the selected item.
- [ ] Support previews rendered directly from source fields.
- [ ] Support safe argv-based preview commands without implicit shell execution.
- [ ] Run command previews asynchronously with debounce, timeout, and stale-result cancellation.
- [ ] Allow preview content to be scrolled.

## Palette Authoring

- [ ] Add `vellum check <palette>` for headless configuration validation.
- [ ] Validate layered configuration, bindings, templates, actions, and built-in source contracts.
- [ ] Add an opt-in flag for running and validating the configured source.
- [ ] Add `vellum palettes list` with descriptions and dependency availability.
- [ ] Provide a bundled palette for discovering and opening installed palettes.

## Website and Documentation

- [x] Create an Astro and Starlight site under `website/`, following the structure used by hwt.
- [x] Build a custom landing page that explains Vellum's command palette, session switcher, agent picker, and file finder use cases.
- [x] Add an animated homepage visualization of fuzzy search, multiline items, live status refresh, filters, and native actions.
- [x] Make the visualization replayable, responsive, accessible, and respectful of reduced-motion preferences.
- [x] Move the user guide into searchable, version-aware documentation with installation, configuration, palette authoring, actions, source, and reference sections.
- [x] Add local website development and production-build mise tasks.
- [x] Build the website in CI and deploy it from `main` with GitHub Pages.
- [x] Add metadata, social preview artwork, canonical URLs, a favicon, and links between the README, landing page, documentation, and repository.

## Sources

- [ ] Add stdin as a source for pipeline composition.
- [ ] Stream NDJSON items into the interface as they arrive.
- [ ] Cancel obsolete source processes when a refresh supersedes them.
- [ ] Preserve responsive search and selection while sources are still loading.

## Selection

- [ ] Add configurable multi-select mode.
- [ ] Support emitting multiple selected values with a documented output format.
- [ ] Support running batch actions over selected items.
- [ ] Clearly indicate selected items independently of the active cursor row.

## Official Palettes

- [ ] Add a Git branch and worktree palette.
- [ ] Add a GitHub pull request palette.
- [ ] Add a zoxide directory palette.
- [ ] Add a process palette.
- [ ] Add a mise task palette.
- [ ] Add palettes for Herdr tabs and recent workspaces.
