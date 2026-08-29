# item-presentation Specification

## Purpose

Render source fields as customizable multiline items with derived values and styles.

## Requirements

### Requirement: Expand item templates

Vellum SHALL render multiline templates, nested fields, derived tokens, animation frames, styles, alignment, search text, and selection values independently.

#### Scenario: Expand a complete multiline item {#ITM-001}

- GIVEN a structured source item and a multiline item template
- WHEN Vellum renders the item
- THEN nested fields, animation, styles, search text, alignment, and output value are resolved

### Requirement: Resolve colors from source data

Vellum SHALL allow item fields to provide dynamic foreground and background colors.

#### Scenario: Source field controls token color {#ITM-002}

- GIVEN a token color that references a source field
- WHEN Vellum renders the token
- THEN the rendered segment uses the field's color value

### Requirement: Rewrite field values for presentation

Vellum SHALL allow ordered exact-match token definitions to replace source field values while preserving an unconditional fallback.

#### Scenario: Replace a known agent with an icon label {#ITM-003}

- GIVEN conditional token definitions mapping known agent names to icon labels followed by an unconditional source-field definition
- WHEN Vellum renders known and unknown agents
- THEN known agents use their configured icon labels and unknown agents retain their source values
