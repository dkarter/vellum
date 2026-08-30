# source-ingestion Specification

## Purpose

Load palette sources and convert their contents into arbitrary structured items.

## Requirements

### Requirement: Accept supported JSON streams

Vellum SHALL accept JSON arrays and newline-delimited JSON objects.

#### Scenario: Parse a JSON array {#SRC-001}

- GIVEN source output containing an array of objects
- WHEN Vellum parses the output
- THEN each object becomes one source item

#### Scenario: Parse NDJSON {#SRC-002}

- GIVEN source output containing objects separated by lines and blank lines
- WHEN Vellum parses the output
- THEN blank lines are ignored and each object becomes one item

#### Scenario: Reject non-object items {#SRC-003}

- GIVEN source output containing a scalar item
- WHEN Vellum parses the output
- THEN parsing fails because template fields require objects

### Requirement: Execute source commands

Vellum SHALL execute configured shell commands and report useful failures.

#### Scenario: Successful source command is parsed {#SRC-004}

- GIVEN a source command that exits successfully with JSON output
- WHEN Vellum runs the command
- THEN its output is returned as source items

#### Scenario: Failed source command reports stderr {#SRC-005}

- GIVEN a source command that writes an error and exits unsuccessfully
- WHEN Vellum runs the command
- THEN the error includes the command status and stderr

### Requirement: Load source items directly from files

Vellum SHALL load `source.file` without invoking a shell command and SHALL select
the parser from the file extension.

#### Scenario: JSON files preserve command output shapes {#SRC-006}

- GIVEN a `.json` source file containing either an array of objects or newline-delimited objects
- WHEN Vellum loads the file
- THEN each object becomes one source item

#### Scenario: JSONC files allow comments {#SRC-007}

- GIVEN a `.jsonc` source file containing an array of objects or newline-delimited objects with comments
- WHEN Vellum loads the file
- THEN comments are ignored and each object becomes one source item

#### Scenario: YAML files use a sequence of mappings {#SRC-008}

- GIVEN a `.yaml` or `.yml` source file whose top level is a sequence of mappings
- WHEN Vellum loads the file
- THEN each mapping becomes one source item

#### Scenario: TOML files use an items array of tables {#SRC-009}

- GIVEN a `.toml` source file containing one or more `[[items]]` tables
- WHEN Vellum loads the file
- THEN each table becomes one source item

#### Scenario: Unsupported file extensions are actionable {#SRC-010}

- GIVEN a source file whose extension is not JSON, JSONC, YAML, YML, or TOML
- WHEN Vellum loads the file
- THEN the error names the path and lists the supported extensions

#### Scenario: Invalid file contents identify the expected shape {#SRC-011}

- GIVEN a supported source file with invalid syntax or the wrong collection or item type
- WHEN Vellum loads the file
- THEN the error names the path and describes the parser or expected collection shape

### Requirement: Resolve file sources from their declaring configuration

Vellum SHALL resolve a relative `source.file` path against the directory of the
palette or global configuration file that declares it. Absolute paths SHALL
remain unchanged.

#### Scenario: Explicit and named palette paths have stable bases {#SRC-012}

- GIVEN either an explicitly loaded palette or a named palette containing a relative `source.file`
- WHEN Vellum resolves the source configuration
- THEN the source path is relative to that palette file's directory regardless of the process working directory

#### Scenario: Inherited global file paths keep the global base {#SRC-013}

- GIVEN a global configuration declaring a relative `source.file` and a palette inheriting that source
- WHEN Vellum merges the configuration layers
- THEN the source path remains relative to the global configuration file's directory

### Requirement: Select exactly one source kind

After global and palette layers merge, Vellum SHALL select exactly one of
`source.cmd`, `source.builtin`, or `source.file`. A source kind explicitly set by
the palette SHALL replace an inherited source kind while retaining unrelated
source options such as `refresh_ms`.

#### Scenario: File source participates in source-kind merging {#SRC-014}

- GIVEN global and palette layers that configure command, built-in, or file source kinds
- WHEN Vellum merges and validates the layers
- THEN a palette source kind replaces inherited kinds and multiple kinds in one layer are rejected

#### Scenario: Refresh reloads file contents {#SRC-015}

- GIVEN a configured file source whose contents change after an initial load
- WHEN Vellum runs the same source configuration again for refresh
- THEN the new file contents are returned as source items
