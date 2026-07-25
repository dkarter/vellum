# terminal-interface Specification

## Purpose

Render a compact responsive terminal interface that composes cleanly inside multiplexer panes and popups.

## Requirements

### Requirement: Render palette content

Vellum SHALL render search state, multiline items, metadata alignment, selection, and result counts.

#### Scenario: Render search items and footer {#UI-001}

- GIVEN a configured palette and source item
- WHEN the terminal frame is drawn
- THEN the search box, multiline item fields, and result count are visible

### Requirement: Configure item chrome

Vellum SHALL disable item borders by default and allow borders and horizontal padding to be configured.

#### Scenario: Item borders can be enabled {#UI-002}

- GIVEN `item.border = true`
- WHEN the list is rendered
- THEN side borders surround the item area

#### Scenario: Item padding aligns content {#UI-003}

- GIVEN a configured horizontal padding
- WHEN the list is rendered
- THEN item text begins after that number of terminal cells

#### Scenario: Excessive padding remains safe {#UI-004}

- GIVEN maximum padding and a narrow terminal
- WHEN the list is rendered
- THEN layout arithmetic saturates without panicking

### Requirement: Keep the search cursor visible

Vellum SHALL horizontally viewport long queries and use a mode-specific terminal cursor.

#### Scenario: Long query scrolls around cursor {#UI-005}

- GIVEN a query wider than the search box
- WHEN the cursor reaches text beyond the visible width
- THEN the displayed query window keeps the cursor inside the input

#### Scenario: Cursor shape reflects input mode {#UI-006}

- GIVEN insert or normal input mode
- WHEN Vellum applies terminal cursor style
- THEN insert uses a steady bar and normal uses a steady block
