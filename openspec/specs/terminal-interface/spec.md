# terminal-interface Specification

## Purpose

Render a compact responsive terminal interface that composes cleanly inside multiplexer panes and popups.

## Requirements

### Requirement: Navigate and style previews

Vellum SHALL render ANSI-colored preview output, scroll it with configurable bindings, optionally show a scrollbar, and allow full, separator, or borderless preview chrome. Results MAY have a titled surrounding box.

#### Scenario: Colored output and scrolling remain independent of selection {#UI-017}

- GIVEN preview output with ANSI colors longer than the preview viewport
- WHEN preview scroll bindings are pressed
- THEN the pane moves without changing the selected item
- AND its colors and scroll position are rendered safely
- AND mouse-wheel input over the preview scrolls it while wheel input over results moves selection

#### Scenario: Preview transitions are immediate and reuse recent results {#UI-019}

- GIVEN a command preview with several selectable items
- WHEN selection changes rapidly or revisits an item
- THEN the old content remains visible until the latest command finishes
- AND a recent result is shown immediately from a bounded cache without rerunning its command
- AND stale workers cannot replace the newest selection

#### Scenario: Preview and results chrome are configurable {#UI-018}

- GIVEN preview border and scrollbar options and a results box title
- WHEN the palette is drawn
- THEN the configured separator or full border, scrollbar, and titled results box are visible

### Requirement: Preview the highlighted item

Vellum SHALL optionally render command output for the selected source item in a themed pane on the left, right, top, or bottom. It SHALL load previews asynchronously, discard stale results, and remain responsive to slow commands.

#### Scenario: Preview follows selection without blocking input {#UI-015}

- GIVEN a configured preview command and multiple items
- WHEN the selection changes while a preview is running
- THEN the pane shows the new item's output instead of stale output
- AND search and navigation remain responsive

#### Scenario: Preview layout adapts to available space {#UI-016}

- GIVEN a preview configured on the left, right, top, or bottom
- WHEN a frame is drawn in a wide or narrow terminal
- THEN the pane uses the configured placement and theme colors when space permits
- AND small terminals keep the item list usable

### Requirement: Render palette content

Vellum SHALL render search state, multiline items, metadata alignment, selection, and result counts.

#### Scenario: Render search items and footer {#UI-001}

- GIVEN a configured palette and source item
- WHEN the terminal frame is drawn
- THEN the search box, multiline item fields, and result count are visible

### Requirement: Configure item chrome

Vellum SHALL disable item borders, vertical spacing, and alternating backgrounds by default and allow each to be configured with horizontal padding.

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

#### Scenario: Item spacing separates list entries {#UI-010}

- GIVEN a configured vertical item spacing
- WHEN the list is rendered
- THEN that number of unselected blank rows separates adjacent items

#### Scenario: Alternating backgrounds distinguish list entries {#UI-011}

- GIVEN a configured alternate item background
- WHEN the visible list is rendered or filtered
- THEN odd visible entries use that background while even entries retain the theme background
- AND selection highlighting remains visually distinct

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

#### Scenario: Repainting does not expose cursor movement {#UI-013}

- GIVEN the search cursor is visible
- WHEN Vellum repaints changed list cells
- THEN the cursor is hidden while the terminal drawing cursor moves
- AND it is positioned in the search input before becoming visible again

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

### Requirement: Separate terminal rendering from machine output

Vellum SHALL write all interactive terminal control and restoration bytes to stderr while reserving stdout for an accepted configured value.

#### Scenario: Accepted output is clean {#OUT-001}

- GIVEN an interactive palette using selection output
- WHEN an item is accepted
- THEN stdout contains only the selected configured value and a trailing newline
- AND terminal rendering and restoration use stderr

#### Scenario: Cancellation has no output {#OUT-002}

- GIVEN a running interactive palette
- WHEN the user cancels
- THEN stdout remains empty

### Requirement: Render before configured sources finish

Vellum SHALL make the interactive terminal responsive while the initial configured source loads.

#### Scenario: Initial configured source loads in the background {#UI-014}

- GIVEN an interactive palette backed by a slow configured source
- WHEN Vellum starts
- THEN it renders a loading state without waiting for the source
- AND it replaces that state with the source items when loading completes
- AND stdin and select-one invocations still load synchronously
