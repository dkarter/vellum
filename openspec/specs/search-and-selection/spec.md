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

Vellum SHALL provide Ctrl-N and Ctrl-P list navigation by default in input and filter modes.

#### Scenario: Readline bindings move list selection {#NAV-001}

- GIVEN multiple visible items
- WHEN Ctrl-N or Ctrl-P is pressed in input or filter mode
- THEN selection moves down or up within the list

### Requirement: Browse by visible pages

Vellum SHALL provide Ctrl-D and Ctrl-U page navigation by default in input and filter modes.

#### Scenario: Page bindings move list selection by the viewport {#NAV-002}

- GIVEN more visible items than fit in the list viewport
- WHEN Ctrl-D or Ctrl-U is pressed in input or filter mode
- THEN selection moves down or up by one viewport of items and remains within the list

### Requirement: Apply configured exact-match filters

Vellum SHALL provide a dedicated filter mode whose configured choices narrow fuzzy-search candidates without changing input mode.

#### Scenario: Filter mode toggles an exact-match predicate {#FIL-001}

- GIVEN a palette with configured filter choices and items with different source values
- WHEN the user enters filter mode and presses a choice key
- THEN only items whose configured source field exactly matches that choice remain visible
- AND the configured all key or the active choice clears it while Escape exits filter mode without changing Vim mode

#### Scenario: Filtered selection preserves accept behavior {#FIL-002}

- GIVEN an active filter with a visible selected item and a configured default action
- WHEN the user presses the accept binding while filter mode is open
- THEN Vellum exits filter mode and requests the default action for that filtered item

#### Scenario: Filtered output-only selection preserves accept behavior {#FIL-003}

- GIVEN an active filter with a visible selected item and no configured default action
- WHEN the user presses the accept binding while filter mode is open
- THEN Vellum exits filter mode and accepts the value of that filtered item
