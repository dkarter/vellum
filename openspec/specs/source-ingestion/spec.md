# source-ingestion Specification

## Purpose

Run palette source commands and convert their output into arbitrary structured items.

## Requirements

### Requirement: Accept supported JSON streams

Vellum SHALL accept JSON arrays and newline-delimited JSON objects.

#### Scenario: Parse a JSON array {#SRC-001}

- GIVEN source output containing an array of objects
- WHEN Vellum parses the output
- THEN each object becomes one source item

#### Scenario: Parse NDJSON {#SRC-002}

- GIVEN source output containing objects separated by lines and blank lines
- WHEN Vellum parses the output
- THEN blank lines are ignored and each object becomes one item

#### Scenario: Reject non-object items {#SRC-003}

- GIVEN source output containing a scalar item
- WHEN Vellum parses the output
- THEN parsing fails because template fields require objects

### Requirement: Execute source commands

Vellum SHALL execute configured shell commands and report useful failures.

#### Scenario: Successful source command is parsed {#SRC-004}

- GIVEN a source command that exits successfully with JSON output
- WHEN Vellum runs the command
- THEN its output is returned as source items

#### Scenario: Failed source command reports stderr {#SRC-005}

- GIVEN a source command that writes an error and exits unsuccessfully
- WHEN Vellum runs the command
- THEN the error includes the command status and stderr
