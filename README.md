<p align="center">
  <img width="140" src="website/src/assets/logo.png" alt="Vellum logo" />
  <br />
  <strong><em>Vellum</em></strong>
</p>

Vellum is a fast, customizable menu for terminal multiplexers. Use it as a command palette, session switcher, agent picker, or fuzzy file finder, with rich multiline items and live data.

**[Website](https://vellum.doriankarter.com/)** · **[Documentation](https://vellum.doriankarter.com/docs/)** · **[Quick start](https://vellum.doriankarter.com/docs/quick-start/)**

## Install

Install a prebuilt release with mise:

```sh
mise use --global github:dkarter/vellum
```

Then install the bundled Herdr and file-finding palettes:

```sh
vlm palettes sync
vlm files
vlm themes # browse and apply a theme with live color previews
```

Vellum writes accepted values to stdout and keeps its interface and diagnostics on stderr, so it composes cleanly with other terminal tools:

```sh
pane_id="$(vlm herdr-agents)" && herdr agent focus "$pane_id"
file="$(vlm files)" && "${EDITOR:-vi}" -- "$file"

```

For opt-in previews, run `vlm examples/files-preview.toml` from a repository checkout with `bat` installed. Configure placements and commands in the [preview guide](https://vellum.doriankarter.com/docs/previews/).

See the [documentation](https://vellum.doriankarter.com/docs/) for installation, configuration, palette authoring, sources, templates, actions, filters, input, frecency, schemas, official palettes, and the CLI reference.

## Develop

Rust 1.88 or newer is required.

```sh
mise install
mise run test
mise run openspec:check
mise run website-build
```

Start the documentation site locally with `mise run website-dev`.

`mise install` also installs the repository's hk Git hooks. Pre-commit checks staged Rust formatting, validates `hk.pkl` when it changes, and scans staged files for secrets; commit messages follow Conventional Commits. Pre-push runs Rust tests, OpenSpec checks, palette/schema metadata tests, or the website build only when relevant files changed. Run `hk check` to check modified files or `hk check --all` to check the whole project.

## License

MIT
