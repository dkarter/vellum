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
