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

### Requirement: Show Vim mode in the footer

Vellum SHALL show the active Vim mode as a colored badge at the left of the footer only when Vim input is enabled.

#### Scenario: Vim mode badge reflects input state {#UI-007}

- GIVEN palettes with Vim input enabled and disabled
- WHEN each terminal frame is drawn
- THEN the enabled frame shows a mode-colored footer badge and the disabled frame shows no mode badge

### Requirement: Respond to terminal resizing

Vellum SHALL redraw its interface when the terminal reports a new size.

#### Scenario: Resize event requests a redraw {#UI-008}

- GIVEN a running Vellum interface
- WHEN Crossterm reports a terminal resize event
- THEN Vellum marks the frame dirty so the next draw uses the new terminal area

### Requirement: Show filter controls and state

Vellum SHALL style the active filter beside the search title and show compact filter controls in the footer.

#### Scenario: Footer reflects filter state {#UI-009}

- GIVEN a palette with configured filters
- WHEN the normal and filter-mode frames are drawn
- THEN the active choice's icon, label, and color appear beside the search title
- AND the footer identifies the filter binding outside filter mode and compactly lists the filter name and keys inside it
