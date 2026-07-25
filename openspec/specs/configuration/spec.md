# configuration Specification

## Purpose

Parse, validate, layer, and describe global and per-palette Vellum configuration.

## Requirements

### Requirement: Apply safe defaults

Vellum SHALL apply documented defaults when optional configuration is omitted.

#### Scenario: Minimal palette receives defaults {#CFG-001}

- GIVEN a palette with only a source command, item template, and selection value
- WHEN Vellum parses the palette
- THEN search, input, keybinding, item padding, and theme defaults are applied

### Requirement: Define derived tokens and styled segments

Vellum SHALL parse conditional token rules, animation settings, and styled template segments.

#### Scenario: Token rules and styled segments parse {#CFG-002}

- GIVEN a palette containing a conditional animated token and styled template segment
- WHEN Vellum parses the palette
- THEN the token conditions, frames, and segment styles are retained

### Requirement: Reject invalid configuration

Vellum SHALL reject configuration that cannot be executed safely or predictably.

#### Scenario: Animated token requires frames {#CFG-003}

- GIVEN a token with an animation rate and no frames
- WHEN Vellum validates the palette
- THEN validation fails with an animation-frames error

#### Scenario: Animation rate has a safe upper bound {#CFG-004}

- GIVEN a token animation rate above 1000 frames per second
- WHEN Vellum validates the palette
- THEN validation fails before frame timing can divide by zero

#### Scenario: Unsupported keybinding is rejected {#CFG-005}

- GIVEN a keybinding value that is neither a supported named key nor one character
- WHEN Vellum parses the palette
- THEN parsing fails with an unsupported-keybinding error

### Requirement: Layer global defaults under palettes

Vellum SHALL recursively merge an optional global configuration beneath the selected palette.

#### Scenario: Palette values override global values {#CFG-006}

- GIVEN global defaults and a palette that overrides one nested value
- WHEN Vellum parses the layered configuration
- THEN the palette value wins and unrelated global values remain

### Requirement: Allow keybindings to be disabled

Vellum SHALL allow all configurable bindings or an individual action to be disabled.

#### Scenario: Binding controls accept false {#CFG-007}

- GIVEN `keybindings.enabled = false` and an individual binding set to `false`
- WHEN Vellum parses the palette
- THEN global binding dispatch and that action are disabled
