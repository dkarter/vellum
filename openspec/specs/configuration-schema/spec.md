# configuration-schema Specification

## Purpose

Provide editor validation and completion metadata for Vellum TOML configuration.

## Requirements

### Requirement: Publish valid JSON Schema

Vellum SHALL include a machine-readable JSON Schema covering global and palette configuration fields.

#### Scenario: Bundled schema is valid JSON {#SCH-001}

- GIVEN the bundled Vellum schema
- WHEN tooling parses it as JSON
- THEN parsing succeeds and identifies a Vellum configuration schema

### Requirement: Associate example palettes with the schema

Vellum SHALL configure Taplo to use one self-contained `schemas/vellum.schema.json` for local examples and the published URL for official palettes.

#### Scenario: Taplo rule references bundled schema {#SCH-002}

- GIVEN the repository Taplo configuration
- WHEN its example-file rule is inspected
- THEN the rule points to `schemas/vellum.schema.json`

#### Scenario: Global configuration uses the Vellum schema {#SCH-003}

- GIVEN Vellum's global example and schema files
- WHEN editor schema associations are inspected
- THEN the global example uses `schemas/vellum.schema.json` and has the same supported settings as a palette

#### Scenario: One schema serves global and palette configuration {#SCH-004}

- GIVEN global and palette examples and the bundled schema
- WHEN their configuration fields are inspected
- THEN both use the same self-contained schema containing the supported option definitions
- AND it has no external schema references

#### Scenario: Shared schema describes palette filters {#SCH-005}

- GIVEN the Vellum configuration schema
- WHEN its filter definitions are inspected
- THEN it describes the filter label, all-items label, separator, mode and all-items bindings, and behavioral and presentation fields for each exact-match choice

#### Scenario: Shared schema describes native actions {#SCH-006}

- GIVEN the Vellum configuration schema
- WHEN its action definitions are inspected
- THEN it describes default and menu controls, named argv or shell commands, interpolated working directories, direct bindings, icons, descriptions, field and cached command availability conditions, and success behavior

#### Scenario: Shared schema describes repeated template segments {#SCH-007}

- GIVEN the Vellum configuration schema
- WHEN its item template segment definitions are inspected
- THEN it describes array iteration, element tokens, separators, uniqueness, styling, searchability, and alignment

#### Scenario: Shared schema describes file-backed sources {#SCH-008}

- GIVEN the Vellum configuration schema
- WHEN their source fields are inspected
- THEN it documents `source.file` alongside command and built-in sources
- AND the file field describes supported data file extensions and declaration-relative paths

#### Scenario: Shared schema describes standard-input sources {#SCH-009}

- GIVEN the Vellum configuration schema
- WHEN their source fields are inspected
- THEN it documents `source.stdin` as a one-shot source alongside command, built-in, and file sources

#### Scenario: Shared schema describes palette working directories {#SCH-010}

- GIVEN the Vellum configuration schema
- WHEN their top-level fields are inspected
- THEN it documents a nonempty working directory path or environment-variable reference
