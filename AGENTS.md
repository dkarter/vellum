# Agent Guidelines

- Near completion, verify the built application interactively by running it in a new Herdr pane and inspecting the rendered output.
- Use test driven development where that helps with designing the interface and outlining cases to be solved
- Use idiomatic Rust code and avoid unsafe or skipping Rust's safety mechanisms
- Prefer smaller modules with a single responsibility rather than huge files that try to do everything, use private functions and extract utils where shared logic need to exist between modules when the abstraction makes sense and will not make the code harder to read by jumping around too much between files
- Always remember to update the config schemas (one for global and one for palettes when changing/adding config options)
- This project uses conventional commits

## OpenSpec

- Specs live at `openspec/specs/<capability>/spec.md`; split them by user-facing capability.
- Give every scenario a stable `{#ABC-001}` ID. Start each linked Rust test name with the lowercase underscore form, for example `abc_001_description`; include additional IDs in the name when one test covers several scenarios.
- Update the scenario first when behavior changes. Run one scenario or every scenario in a spec with `mise run test:spec <ABC-001|capability|spec-path>`.
- Run `mise run openspec:check` to validate OpenSpec and enforce bidirectional links: every scenario needs a test, every test needs a known scenario, and IDs must be unique.
