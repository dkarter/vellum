# frecency Specification

## Purpose

Rank previously selected palette entries by frequency and recency using bounded XDG data storage.

## Requirements

### Requirement: Score frequency and recency

Vellum SHALL combine bounded selection frequency with time-decay buckets and exact recent-use ordering.

#### Scenario: Recent frequent entries receive higher scores {#FRC-001}

- GIVEN selection records with different access counts and ages
- WHEN Vellum calculates their frecency ranks
- THEN recent frequent entries score above older or less-used entries

### Requirement: Persist history per palette

Vellum SHALL store selection history by canonical palette identity and item value in an XDG SQLite database.

#### Scenario: Frecency data round-trips by palette {#FRC-002}

- GIVEN selections recorded for different palettes
- WHEN history is saved and loaded again
- THEN each palette retains only its own entry records

### Requirement: Bound persistent history

Vellum SHALL limit the number of retained records per palette.

#### Scenario: Lowest-ranked records are pruned {#FRC-003}

- GIVEN a palette exceeds its configured history limit
- WHEN another selection is recorded
- THEN the lowest-ranked records are removed

### Requirement: Hoist previously selected matches

Vellum SHALL place selected entries before unseen entries while preserving fuzzy quality and source order as fallback behavior.

#### Scenario: Selected entries rank by frecency and exact recency {#FRC-004}

- GIVEN matching items with unseen, older, and recently selected values
- WHEN Vellum orders visible results
- THEN selected values are hoisted by score and latest access before unseen values

### Requirement: Configure frecency per palette

Vellum SHALL enable frecency by default and allow global defaults or an individual palette to disable it and bound retained entries.

#### Scenario: Frecency settings parse and layer {#FRC-005}

- GIVEN global frecency settings and a palette override
- WHEN Vellum parses the layered configuration
- THEN the palette setting wins and documented defaults fill omitted values

### Requirement: Store data at the XDG location

Vellum SHALL use `VELLUM_DATA`, then `XDG_DATA_HOME`, then the standard local data fallback.

#### Scenario: Data root honors environment precedence {#FRC-006}

- GIVEN explicit Vellum or XDG data roots
- WHEN Vellum resolves its data directory
- THEN the highest-precedence configured root is used with a `vellum` directory where appropriate

### Requirement: Record accepted selections

Vellum SHALL update and persist frecency only after an item is accepted.

#### Scenario: Accepted value is recorded before output {#FRC-007}

- GIVEN frecency is enabled for the active palette
- WHEN the user accepts a selected value
- THEN Vellum records and saves that palette-value access before printing it

### Requirement: Preserve concurrent selections

Vellum SHALL serialize updates from overlapping processes without losing either selection.

#### Scenario: Concurrent sessions preserve both updates {#FRC-008}

- GIVEN two Vellum sessions opened against the same data store
- WHEN each records a different selected value
- THEN both records remain in persistent history

### Requirement: Identify palettes consistently

Vellum SHALL scope history by the canonical resolved palette path regardless of invocation form.

#### Scenario: Palette identity uses resolved canonical path {#FRC-009}

- GIVEN a named or explicit invocation resolving to the same palette file
- WHEN Vellum determines the frecency scope
- THEN both invocations use the same canonical path identity
