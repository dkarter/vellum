# native-actions Specification

## Purpose

Run explicit palette commands for selected items without compromising selection output or terminal safety.

## Requirements

### Requirement: Configure safe selection actions

Vellum SHALL support named actions with argv-array commands, selected-item field interpolation, optional explicit shell commands, keybindings, selected-field and command availability conditions, and exit or refresh success behavior.

#### Scenario: Action configuration parses and validates {#ACT-001}

- GIVEN named argv and explicit-shell actions with a default action and menu binding
- WHEN Vellum parses the palette
- THEN command type, bindings, availability conditions, and success behavior are retained and invalid references or ambiguous commands are rejected

### Requirement: Preserve output-only palettes

Vellum SHALL accept and print the configured item value when no default action is configured.

#### Scenario: Enter remains selection output without a default action {#ACT-002}

- GIVEN a palette with no configured default action
- WHEN Enter is pressed on an item
- THEN Vellum accepts the item's configured output value

### Requirement: Dispatch configured actions

Vellum SHALL request the configured default action on Enter and named actions through their direct bindings.

#### Scenario: Default and direct keys request actions {#ACT-003}

- GIVEN a selected item and configured default and bound actions
- WHEN Enter or an action binding is pressed
- THEN Vellum requests the corresponding action for that item without accepting output

### Requirement: Offer a quick-action menu

Vellum SHALL show a compact in-process menu of actions for the selected item, including configured icons and descriptions, and support fuzzy filtering, navigation, cancellation, and selection without closing the palette.

#### Scenario: Quick-action menu navigates and selects {#ACT-004}

- GIVEN multiple configured actions
- WHEN the menu binding is pressed and the user navigates and accepts
- THEN the highlighted action is requested for the current item
- AND Escape closes the menu while leaving Vellum running

#### Scenario: Quick-action menu fuzzy-filters metadata {#ACT-009}

- GIVEN available actions with configured names, labels, icons, and descriptions
- WHEN the user types a fuzzy query in the action menu
- THEN actions are filtered and ranked by their textual metadata
- AND arrow keys, Ctrl-N, and Ctrl-P continue to navigate the filtered actions
- AND each configured icon is separated from its label by one space
- AND action items use a one-cell horizontal gutter without an extra blank item row

### Requirement: Resolve command-gated availability without blocking input

Vellum SHALL resolve optional argv command availability probes outside the input and rendering paths, treat only a zero exit status as available, and cache equivalent probe results for the configured duration.

#### Scenario: Command-gated actions resolve asynchronously and share cached results {#ACT-011}

- GIVEN actions with equivalent availability commands and interpolated working directories
- WHEN the action menu opens before an uncached probe completes
- THEN the menu opens immediately while those actions remain hidden
- AND a zero exit status makes the actions available while a nonzero status leaves them hidden
- AND equivalent probes reuse the cached result until its configured duration expires
- AND a probe that exceeds its configured timeout is terminated without blocking input or other probes

### Requirement: Execute actions safely

Vellum SHALL execute argv commands directly without a shell, interpolate selected scalar fields by argument and working directory, and report failed or invalid actions without exiting.

#### Scenario: Argv interpolation preserves argument boundaries {#ACT-005}

- GIVEN an argv action referencing a selected field containing shell metacharacters or spaces
- WHEN the action is prepared and run
- THEN the field remains one literal process argument and no shell interpretation occurs

#### Scenario: Failed action remains visible {#ACT-006}

- GIVEN an action command that exits unsuccessfully
- WHEN it runs
- THEN Vellum remains open and displays the exit status and stderr detail

#### Scenario: Action working directory interpolates safely {#ACT-010}

- GIVEN an action working directory that references a selected scalar field
- WHEN the argv action runs
- THEN the process uses that directory without invoking a shell
- AND a missing, null, or non-scalar directory field fails before spawning

### Requirement: Refresh after successful actions

Vellum SHALL rerun the source after a successful refresh action while preserving query and selection identity when possible and choosing the nearest remaining position otherwise.

#### Scenario: Refresh preserves query and selected identity {#ACT-007}

- GIVEN a query and selected item
- WHEN a successful action refreshes a reordered source that still contains the item
- THEN the query and selected item remain unchanged

#### Scenario: Refresh selects a nearby remaining item after deletion {#ACT-008}

- GIVEN a selected item between other visible items
- WHEN a successful action refresh removes it
- THEN the query remains unchanged and the item at the nearest remaining list position is selected
