# input-editing Specification

## Purpose

Edit search queries with readline and Vim conventions while retaining a reliable cancellation path.

## Requirements

### Requirement: Support Vim mode transitions

Vellum SHALL start in configured Vim mode and use Escape to transition from insert to normal before closing from normal.

#### Scenario: Escape transitions then cancels {#INP-001}

- GIVEN Vim input starts in insert mode
- WHEN Escape is pressed twice
- THEN the first press enters normal mode and the second cancels Vellum

### Requirement: Support readline editing

Vellum SHALL support cursor movement, line boundaries, previous-word deletion, and list navigation through default readline bindings.

#### Scenario: Readline bindings edit the query {#INP-002}

- GIVEN a populated search query
- WHEN Ctrl-F, Ctrl-B, Ctrl-A, Ctrl-E, or Ctrl-W is pressed
- THEN the cursor or query changes according to readline behavior

### Requirement: Support basic Vim editing

Vellum SHALL support basic normal-mode movement, deletion, insertion transitions, and list browsing.

#### Scenario: Vim normal mode edits and returns to insert {#INP-003}

- GIVEN a query in Vim normal mode
- WHEN movement, deletion, and insertion commands are used
- THEN the query and mode follow basic Vim behavior

#### Scenario: Escape cancels directly when Vim is disabled {#INP-004}

- GIVEN Vim mode is disabled
- WHEN Escape is pressed
- THEN Vellum cancels immediately through the default cancel binding

#### Scenario: End deletion keeps a valid normal cursor {#INP-005}

- GIVEN a non-empty query in Vim normal mode
- WHEN end-of-line movement and repeated deletion are used
- THEN the cursor remains on the final grapheme until the query is empty

### Requirement: Edit visible Unicode characters atomically

Vellum SHALL move and delete by grapheme cluster rather than splitting visible characters.

#### Scenario: Combining and joined emoji delete as graphemes {#INP-006}

- GIVEN a combining character or joined emoji sequence
- WHEN the user deletes the preceding visible character
- THEN the complete grapheme cluster is removed

### Requirement: Always provide emergency cancellation

Vellum SHALL reserve Ctrl-C as an unconditional cancellation action.

#### Scenario: Ctrl-C ignores modes and disabled bindings {#CAN-001}

- GIVEN configurable keybindings are disabled in any input mode
- WHEN Ctrl-C is pressed
- THEN Vellum cancels without emitting a selected value
