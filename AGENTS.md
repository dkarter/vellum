# Agent Guidelines

- Near completion, verify the built application interactively by running it in a new Herdr pane and inspecting the rendered output.
- Use test driven development where that helps with designing the interface and outlining cases to be solved
- Use idiomatic Rust code and avoid unsafe or skipping Rust's safety mechanisms
- Prefer smaller modules with a single responsibility rather than huge files that try to do everything, use private functions and extract utils where shared logic need to exist between modules when the abstraction makes sense and will not make the code harder to read by jumping around too much between files
