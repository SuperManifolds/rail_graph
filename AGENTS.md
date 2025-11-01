# Claude Code Preferences

## Git Commits

- Write clear, professional commit messages that focus on what changed and why
- Use conventional commit style when appropriate

## Communication Style

- Be concise and direct
- Avoid unnecessary preamble or postamble

## Code Changes

- Always run `cargo check` and `cargo clippy` after making code changes (cargo clippy --all-targets -- -D warnings)
- When asked to commit, create a clear commit message without asking for confirmation
- Focus on the technical implementation rather than over-explaining what was done
- When implementing a new utility function make sure it is not already implemented elsewhere in the code base
- When adding new properties to models add them to the bottom so as to avoid breaking deserialization of existing projects

## Rust Code Quality

- Avoid excessive nesting in functions (prefer early returns, extract helper functions)
- Do not silence clippy warnings without expressed consent, address the problem instead
- Keep functions small and focused on a single responsibility
- Follow Rust naming conventions and idiomatic patterns
- Structure the project according to Rust conventions (proper module organization, appropriate use of traits, etc.)
- Address clippy warnings and suggestions when they improve code quality. Do not attempt to silence a lint warning without asking.
- When making a change to existing code that will negatively affect time complexity you must request permission.
- Use constants at the top of the file for magic numbers or layout and style choices like color, width, spacing, etc.
- Prefer declarative over imperative code when sensible (use iterators, functional patterns, etc.)
- Avoid unnecessary suffixes to files or structs like 'view', 'component', 'manager', etc.
- Do not create a new version of an existing function if it makes the old function redundant, just modify the existing function
- Do not use `_` prefixes or `#[allow(dead_code)]` to silence unused code warnings - just remove code that is no longer used

## Components
- Avoid code inside components that is not directly related to UI. Model code should go in models/
- Avoid large amount of function code inside event handlers, extract into functions
- Prefer HTML5 semantic tags like <header> and <section> avoid div soup
- Never use style tags except to apply a temporary reactive effect like moving an element

