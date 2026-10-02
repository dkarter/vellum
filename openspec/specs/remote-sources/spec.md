# Remote sources

## Purpose

Load filtered remote pages without blocking the interface, optionally reusing a local cache.

## Requirements

### Requirement: Load remote pages on demand

Vellum SHALL load opt-in remote sources asynchronously using filter values and opaque cursors, without changing one-shot source behavior.

#### Scenario: Navigate to fetch another page {#REM-001}

- GIVEN an opt-in remote source returning items and an opaque next cursor
- WHEN downward navigation reaches the end of the visible list
- THEN the next page loads asynchronously with a loading indicator
- AND items append without duplicate values or losing the selected item
- AND exhausted sources stop requesting pages

#### Scenario: Change the remote filter {#REM-002}

- GIVEN an in-flight page for a remote filter
- WHEN the active choice changes
- THEN the old request is cancelled and its result cannot replace the new filter's items
- AND command sources receive VELLUM_FILTER, VELLUM_CURSOR, and VELLUM_PAGE_SIZE environment values
- AND command pages use a JSON object containing items and an optional next_cursor
- AND fuzzy search continues to operate locally over loaded items
- AND direct choice shortcuts remain usable even when a filter is empty

#### Scenario: Remote default-filter availability controls cycling {#REM-007}

- GIVEN an opt-in remote palette that probes default-filter availability
- WHEN the availability results arrive
- THEN empty default categories are dimmed and skipped by nonempty Tab cycling
- AND direct shortcuts still select empty choices
- AND the initial filter stays selected even when empty
- AND availability is based on the default categories, not the fuzzy query or only the active loaded page
- AND availability is cached for the source cache TTL and refreshed in the background when stale

#### Scenario: Debounced remote search augments immediate local matches {#REM-008}

- GIVEN a remote palette with server search enabled
- WHEN the user edits the fuzzy query
- THEN loaded items are filtered immediately
- AND after the configured debounce interval the source receives the latest query
- AND stale requests are cancelled and cannot overwrite a newer query's results
- AND server results augment loaded matches without resetting the query or selected identity
- AND pagination and cache entries are scoped to the active filter and server query
- AND clearing the query restores default pagination without a debounce delay
- AND a palette without server search enabled keeps local-only fuzzy search

### Requirement: Cache remote pages with a freshness window

Vellum SHALL optionally reuse bounded local snapshots for a configured TTL and refresh expired snapshots without clearing visible data.

#### Scenario: Fresh and stale cache startup {#REM-003}

- GIVEN cached pages scoped to working directory, source configuration, filter, and GitHub account where applicable
- WHEN the palette opens before its cache TTL expires
- THEN cached items and pagination state appear without a network request
- WHEN the cache is expired
- THEN cached items remain visible while a fresh first page loads in the background
- AND pagination does not extend the first page's freshness window
- AND missing or corrupt cache data behaves as a cache miss

#### Scenario: Remote errors keep the palette usable {#REM-004}

- GIVEN a remote request that fails
- WHEN its result arrives
- THEN the palette keeps existing items and displays an error rather than exiting
- AND downward navigation retries the failed first-page refresh or next-page request

#### Scenario: Select-one waits for an exhausted remote source {#REM-005}

- GIVEN a select-one invocation whose first remote page contains one item
- WHEN the source has another page
- THEN Vellum opens the interactive palette rather than accepting an incomplete result set
- WHEN the source is exhausted with exactly one item
- THEN automatic selection retains its existing behavior

#### Scenario: Persistent cache stays bounded {#REM-006}

- GIVEN remote cache snapshots
- WHEN writing them
- THEN serialization, atomic replacement, and pruning run outside the UI thread with a bounded queue
- AND individual snapshots are capped at eight MiB and the cache directory retains at most one hundred snapshots after pruning
- AND private cache files are owner-readable and writable on Unix
