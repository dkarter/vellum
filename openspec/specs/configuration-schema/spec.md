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

Vellum SHALL configure Taplo to use the bundled schema for example TOML files.

#### Scenario: Taplo rule references bundled schema {#SCH-002}

- GIVEN the repository Taplo configuration
- WHEN its example-file rule is inspected
- THEN the rule points to `schemas/vellum.schema.json`

#### Scenario: Global configuration has a dedicated schema {#SCH-003}

- GIVEN Vellum's global example and schema files
- WHEN editor schema associations are inspected
- THEN global configuration uses `schemas/global.schema.json` and inherits all supported settings

#### Scenario: Global and palette schemas share option definitions {#SCH-004}

- GIVEN the global and palette schema entry points
- WHEN their configuration fields are inspected
- THEN both reference one bundled schema containing the shared option definitions
- AND local schema loading resolves that reference locally rather than rebasing it to the published schema URL

#### Scenario: Shared schema describes palette filters {#SCH-005}

- GIVEN the shared configuration option schema
- WHEN its filter definitions are inspected
- THEN it describes the filter label, mode and all-items bindings, and behavioral and presentation fields for each exact-match choice

#### Scenario: Shared schema describes native actions {#SCH-006}

- GIVEN the shared configuration option schema
- WHEN its action definitions are inspected
- THEN it describes default and menu controls, named argv or shell commands, interpolated working directories, direct bindings, icons, descriptions, field and cached command availability conditions, and success behavior

#### Scenario: Shared schema describes repeated template segments {#SCH-007}

- GIVEN the shared configuration option schema
- WHEN its item template segment definitions are inspected
- THEN it describes array iteration, element tokens, separators, uniqueness, styling, searchability, and alignment

#### Scenario: Shared schema describes file-backed sources {#SCH-008}

- GIVEN the global and palette schema entry points and their shared option definitions
- WHEN their source fields are inspected
- THEN both schemas document `source.file` alongside command and built-in sources
- AND the file field describes supported data file extensions and declaration-relative paths
