# spec-test-linkage Specification

## Purpose

Keep executable Rust tests bidirectionally linked to stable OpenSpec scenarios.

## Requirements

### Requirement: Enforce bidirectional scenario links

Vellum SHALL fail validation when scenarios lack tests, tests lack scenario tags, tags are unknown, or IDs are duplicated.

#### Scenario: Repository spec links are complete {#META-001}

- GIVEN OpenSpec scenario headings and Rust test function names
- WHEN the spec-link checker runs
- THEN every active test and scenario has a valid bidirectional link

### Requirement: Run tests for a scenario or spec file

Vellum SHALL provide one command that resolves a scenario ID or capability spec to Cargo test filters.

#### Scenario: Spec runner resolves all file scenarios {#META-002}

- GIVEN a capability name or spec path
- WHEN the spec-test runner resolves it
- THEN it emits one Cargo-compatible filter for every scenario in that file
