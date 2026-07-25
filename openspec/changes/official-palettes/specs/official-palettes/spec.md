## ADDED Requirements

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

### Requirement: Provide a Herdr workspace palette

Vellum SHALL bundle a workspace switcher showing active agents and their states for each Herdr workspace.

#### Scenario: Workspace palette exposes agent context {#PAL-003}

- GIVEN Herdr reports workspaces and active agents
- WHEN the workspace palette source is evaluated
- THEN each workspace item includes agent identity and state and selects a workspace identifier

### Requirement: Provide a Herdr agent palette

Vellum SHALL bundle an agent switcher showing each active agent's status.

#### Scenario: Agent palette exposes status {#PAL-004}

- GIVEN Herdr reports active agents
- WHEN the agent palette source is evaluated
- THEN each item includes agent status and selects an agent identifier

### Requirement: Provide a file finder palette

Vellum SHALL bundle an `fd`-backed file finder with colorful Nerd Font filetype icons.

#### Scenario: File palette maps paths to icons and values {#PAL-005}

- GIVEN `fd` returns files with known and unknown extensions
- WHEN the file palette source is evaluated
- THEN each item has an appropriate colored icon and selects its path
