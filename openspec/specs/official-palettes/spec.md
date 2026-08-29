# official-palettes Specification

## Purpose
Distribute maintained Herdr and file-finder palettes safely through the Vellum binary.
## Requirements
### Requirement: Safely synchronize official palettes

Vellum SHALL install bundled palettes into the XDG palette directory without silently replacing user files.

#### Scenario: Default sync preserves existing palettes {#PAL-001}

- GIVEN an official palette path already exists
- WHEN the user syncs official palettes without overwrite
- THEN the existing file remains unchanged and the conflict is reported

#### Scenario: Explicit overwrite updates palettes {#PAL-002}

- GIVEN an official palette path already exists
- WHEN the user syncs with explicit overwrite enabled
- THEN the bundled palette replaces the destination file

#### Scenario: Sync installs every bundled palette {#PAL-006}

- GIVEN an empty injected configuration root
- WHEN official palettes are synchronized
- THEN every bundled palette is installed with its embedded contents

#### Scenario: Overwrite refuses symbolic-link targets {#PAL-009}

- GIVEN an official palette destination is a symbolic link
- WHEN explicit overwrite is requested
- THEN synchronization fails without modifying the link target

### Requirement: Provide a Herdr workspace palette

Vellum SHALL bundle a compact workspace switcher showing workspace identity, checkout path, and aggregate status.

#### Scenario: Workspace palette exposes agent context {#PAL-003}

- GIVEN Herdr reports workspaces and active agents
- WHEN the workspace palette source is evaluated
- THEN each workspace item includes agent identity and state and selects a workspace identifier

#### Scenario: Workspace palette uses an aligned two-line layout {#PAL-013}

- GIVEN the bundled Herdr workspace palette
- WHEN a workspace item is rendered
- THEN its name and status share the first row, its checkout path uses the second row, and numeric and agent-detail metadata are hidden
- AND the workspace name aligns with the folder icon while focus is indicated by color instead of a leading glyph

#### Scenario: Workspace palette provides native focus and removal actions {#PAL-015}

- GIVEN the bundled Herdr workspace palette
- WHEN its actions are inspected
- THEN Enter focuses the selected workspace and exits
- AND the quick-action menu can remove its HWT worktree and refresh the workspace source

### Requirement: Provide a Herdr agent palette

Vellum SHALL bundle an agent switcher showing each active agent's status.

#### Scenario: Agent palette exposes status {#PAL-004}

- GIVEN Herdr reports active agents
- WHEN the agent palette source is evaluated
- THEN each item includes agent status and selects an agent identifier

#### Scenario: Agent pane ID is output-only {#PAL-010}

- GIVEN the bundled Herdr agent palette
- WHEN an agent item is rendered
- THEN the pane ID remains the selection value but is not displayed in the item template

#### Scenario: Agent palette filters by lifecycle state {#PAL-014}

- GIVEN the bundled Herdr agent palette
- WHEN its filter configuration is parsed
- THEN working, done, idle, blocked, and unknown states have dedicated filter choices

### Requirement: Provide a file finder palette

Vellum SHALL bundle an `fd`-backed file finder with colorful Nerd Font filetype icons.

#### Scenario: File palette maps paths to icons and values {#PAL-005}

- GIVEN `fd` returns files with known and unknown extensions
- WHEN the file palette source is evaluated
- THEN each item has an appropriate colored icon and selects its path

#### Scenario: File palette uses a compact path layout {#PAL-011}

- GIVEN the bundled file finder palette
- WHEN a file item is rendered
- THEN its icon, parent path, and bold filename appear on one line with compact spacing

### Requirement: Keep bundled palettes compatible

Vellum SHALL validate bundled palettes against its configuration and built-in source contracts.

#### Scenario: Every bundled palette parses {#PAL-007}

- GIVEN the embedded official palette assets
- WHEN each palette is parsed as Vellum configuration
- THEN all palettes pass validation

#### Scenario: Templates match built-in source fields {#PAL-008}

- GIVEN representative output from each built-in source
- WHEN each official template and selection value is resolved
- THEN referenced fields exist and searchable text and output values are non-empty

#### Scenario: Bundled palettes identify their search inputs {#PAL-012}

- GIVEN the embedded official palette assets
- WHEN their search configuration is parsed
- THEN each palette provides a descriptive input title
