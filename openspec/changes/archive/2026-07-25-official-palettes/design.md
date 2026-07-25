# Design: Official palettes

## Palette distribution

Embed or package official palette assets with the binary so installation does not depend on a source checkout. A dedicated CLI command resolves the XDG destination and writes each requested palette.

## Safety

The default sync operation creates missing files and reports conflicts. An explicit overwrite option replaces existing official palette paths. Filesystem logic accepts an injected destination in tests.

## Data sources

Herdr palettes use the installed Herdr CLI's current JSON output and transform only where Vellum templates cannot represent aggregation. The file palette uses `fd` and a maintained Nerd Font icon mapping derived from a reputable upstream source.
