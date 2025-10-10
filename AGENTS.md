# Claude Code Preferences

## Git Commits

- Write clear, professional commit messages that focus on what changed and why
- Use conventional commit style when appropriate

## Communication Style

- Be concise and direct
- Avoid unnecessary preamble or postamble
- Get straight to the point when answering questions
- Only provide detailed explanations when asked or when the task is complex

## Code Changes

- Make changes efficiently and test them
- When asked to commit, create a clear commit message without asking for confirmation
- Focus on the technical implementation rather than over-explaining what was done

## Rust Code Quality

- Always run `cargo check` and `cargo clippy` after making code changes
- Avoid excessive nesting in functions (prefer early returns, extract helper functions)
- Keep functions small and focused on a single responsibility
- Follow Rust naming conventions and idiomatic patterns
- Structure the project according to Rust conventions (proper module organization, appropriate use of traits, etc.)
- Address clippy warnings and suggestions when they improve code quality
- Use constants at the top of the file for magic numbers or layout and style choices like color, width, spacing, etc.
- Prefer declarative over imperative code when sensible (use iterators, functional patterns, etc.)
- Avoid code inside components that is not directly related to UI. Model code should go in models/
- Avoid unnecessary suffixes to files or structs like 'view', 'component', 'manager', etc.
- Do not create a new version of an existing function if it makes the old function redundant, just modify the existing function
- Do not use `_` prefixes or `#[allow(dead_code)]` to silence unused code warnings - just remove code that is no longer used
