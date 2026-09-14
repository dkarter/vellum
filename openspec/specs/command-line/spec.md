# command-line Specification

## Purpose

Resolve palette names and paths, expose help, and reject ambiguous invocation.

## Requirements

### Requirement: Accept explicit palette paths

Vellum SHALL preserve explicit TOML paths supplied by the user.

#### Scenario: Explicit path is selected {#CLI-001}

- GIVEN a TOML path argument
- WHEN CLI arguments are parsed
- THEN that argument is retained as the requested palette

### Requirement: Expose informational commands

Vellum SHALL print help and version information without loading a palette.

#### Scenario: Help and version bypass palette loading {#CLI-002}

- GIVEN `--help` or `--version`
- WHEN CLI arguments are parsed
- THEN an informational command is returned

### Requirement: Reject extra positional arguments

Vellum SHALL accept at most one palette name or path.

#### Scenario: Extra arguments fail {#CLI-003}

- GIVEN more than one positional argument
- WHEN CLI arguments are parsed
- THEN parsing fails with an argument-count error

### Requirement: Resolve named palettes from XDG configuration

Vellum SHALL resolve names beneath the XDG palette directory while preserving explicit paths.

#### Scenario: Names and paths resolve differently {#CLI-004}

- GIVEN one palette name and one explicit path
- WHEN each is resolved
- THEN the name uses `palettes/<name>.toml` and the path remains unchanged

#### Scenario: Missing argument selects default palette {#CLI-005}

- GIVEN no palette argument
- WHEN CLI arguments are parsed
- THEN the named palette `default` is selected

### Requirement: Synchronize official palettes

Vellum SHALL expose safe and explicit-overwrite variants of the official palette sync command.

#### Scenario: Palette sync commands parse {#CLI-006}

- GIVEN `palettes sync` with or without `--overwrite`
- WHEN CLI arguments are parsed
- THEN Vellum selects safe synchronization or explicit overwrite respectively

### Requirement: Override a palette source from standard input

Vellum SHALL provide one-shot command-line source overrides for JSON or NDJSON,
plain lines wrapped in a named field, and JSON transformed by an external `jq`
filter. Simple `TARGET=SOURCE` field mappings SHALL be repeatable.

#### Scenario: Standard-input source flags parse {#CLI-007}

- GIVEN `--stdin`, `--lines FIELD`, or `--jq FILTER` and optional `--field TARGET=SOURCE` arguments
- WHEN CLI arguments are parsed
- THEN Vellum retains the palette and the requested one-shot source transformation
- AND conflicting modes, malformed mappings, and mappings without a standard-input mode are rejected

#### Scenario: Stdin without a palette uses a generic finder {#CLI-008}

- GIVEN `vellum --stdin` without a palette argument
- WHEN CLI arguments are parsed and plain lines are loaded
- THEN Vellum uses an embedded palette that displays and returns each line

### Requirement: Select an unambiguous result without opening the menu

Vellum SHALL provide opt-in `-1` and `--select-1` arguments that accept the initial result when exactly one item is available.

#### Scenario: Select-one argument parses {#CLI-009}

- GIVEN `--select-1` or `-1` with a named palette or standard-input source
- WHEN CLI arguments are parsed
- THEN Vellum retains the select-one request for that palette invocation

### Requirement: Select the process working directory

Vellum SHALL accept a `--cwd PATH` option, resolve and enter that directory
before loading the palette source, and report a clear error when the directory
cannot be used. Source commands and actions without their own `cwd` SHALL
inherit the selected directory, while an action-level `cwd` SHALL override it.

#### Scenario: Process working directory controls sources and actions {#CLI-010}

- GIVEN a palette whose source and action depend on relative paths
- WHEN Vellum runs the palette with `--cwd PATH`
- THEN the source and an action without `cwd` run in `PATH`
- AND an action-level `cwd` overrides `PATH`

#### Scenario: Invalid process working directory fails clearly {#CLI-011}

- GIVEN a missing or non-directory path
- WHEN Vellum runs a palette with that path as `--cwd`
- THEN Vellum exits before running the source and identifies the working-directory error
