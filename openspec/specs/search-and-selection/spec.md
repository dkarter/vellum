# search-and-selection Specification

## Purpose

Fuzzy-filter palette items, browse results, accept values, and preserve selection during live updates.

## Requirements

### Requirement: Fuzzy-match searchable item text

Vellum SHALL rank matching items while excluding items that do not fuzzy-match the query.

#### Scenario: Fuzzy query filters and ranks items {#SEA-001}

- GIVEN rendered items with different searchable text
- WHEN a fuzzy query is evaluated
- THEN matching item indices are returned in score order

#### Scenario: Query can be accepted as a selected value {#SEA-002}

- GIVEN an interactive palette with multiple items
- WHEN the user filters to an item and accepts it
- THEN Vellum returns that item's configured value

### Requirement: Preserve stable selection across refreshes

Vellum SHALL preserve selection by configured output value when refreshed items reorder.

#### Scenario: Refresh keeps selected value {#REF-001}

- GIVEN an item is selected
- WHEN refreshed source data moves that item to another position
- THEN the same output value remains selected

### Requirement: Browse with readline list bindings

Vellum SHALL provide Ctrl-N and Ctrl-P list navigation by default.

#### Scenario: Readline bindings move list selection {#NAV-001}

- GIVEN multiple visible items
- WHEN Ctrl-N or Ctrl-P is pressed
- THEN selection moves down or up within the list
