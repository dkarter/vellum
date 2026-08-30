<p align="center">
  <img width="140" src="website/src/assets/logo.png" alt="Vellum logo" />
  <br />
  <strong><em>Vellum</em></strong>
</p>

Vellum is a fast, customizable menu for terminal multiplexers. Use it as a command palette, session switcher, agent picker, or fuzzy file finder, with rich multiline items and live data.

**[Website](https://dkarter.github.io/vellum/)** · **[Documentation](https://dkarter.github.io/vellum/docs/)** · **[Quick start](https://dkarter.github.io/vellum/docs/quick-start/)**

## Install

Install a prebuilt release with mise:

```sh
mise use --global github:dkarter/vellum
```

Then install the bundled Herdr and file-finding palettes:

```sh
vellum palettes sync
vellum files
```

Vellum writes accepted values to stdout and keeps its interface and diagnostics on stderr, so it composes cleanly with other terminal tools:

```sh
pane_id="$(vellum herdr-agents)" && herdr agent focus "$pane_id"
file="$(vellum files)" && "${EDITOR:-vi}" -- "$file"
```

See the [documentation](https://dkarter.github.io/vellum/docs/) for installation, configuration, palette authoring, sources, templates, actions, filters, input, frecency, schemas, official palettes, and the CLI reference.

## Develop

Rust 1.88 or newer is required.

```sh
mise install
mise run test
mise run openspec:check
mise run website-build
```

Start the documentation site locally with `mise run website-dev`.

## License

MIT
