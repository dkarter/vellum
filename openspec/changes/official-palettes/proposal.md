# Change: Ship official palettes

## Why

Users should be able to install polished palettes for common Herdr and file-navigation workflows without recreating source commands, templates, or icon mappings.

## What Changes

- Bundle maintained Herdr workspace, Herdr agent, and file-finder palettes.
- Add a safe command for syncing bundled palettes into the XDG palette directory.
- Preserve user edits unless overwrite is explicitly requested.

## Non-goals

- Automatically modifying shell or multiplexer configuration.
- Replacing user palettes during normal Vellum startup.
