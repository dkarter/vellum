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
