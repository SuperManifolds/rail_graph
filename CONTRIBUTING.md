# Contributing to RailGraph

Thank you for your interest in contributing to RailGraph! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- **Rust** 1.76.0 or later
- **Trunk** (for building and serving)
- **wasm32-unknown-unknown** target

### Development Setup

1. **Install Rust and Trunk**

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install trunk
cargo install --locked trunk

# Add WebAssembly target
rustup target add wasm32-unknown-unknown
```

2. **Fork and Clone**

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/nimby_graph.git
cd nimby_graph
```

3. **Run the Development Server**

```bash
trunk serve
```

The application will be available at `http://localhost:8080` with hot reload enabled.

## Development Workflow

### Running Tests

```bash
# Run all tests
cargo test

# Run WASM tests (requires wasm-pack)
wasm-pack test --headless --chrome
```

### Code Quality

Before submitting changes, ensure your code passes quality checks:

```bash
# Check for compilation errors
cargo check

# Run clippy (treat warnings as errors)
cargo clippy --all-targets -- -D warnings
```

### Code Style

This project follows the Rust conventions outlined in `AGENTS.md`:

- Run `cargo check` and `cargo clippy` after making changes
- Avoid excessive nesting (prefer early returns, extract helper functions)
- Keep functions small and focused on a single responsibility
- Follow Rust naming conventions and idiomatic patterns
- Use constants at the top of files for magic numbers, colors, dimensions, etc.
- Prefer declarative over imperative code (iterators, functional patterns)
- Avoid code inside components that is not UI-related (model code goes in `models/`)
- Do not use `_` prefixes or `#[allow(dead_code)]` to silence warnings - remove unused code

## Making Changes

### Branch Naming

Create a branch using the format: `githubusername/<issue-id>-description`

```bash
git checkout -b yourname/123-add-station-filtering
# or for bug fixes
git checkout -b yourname/456-fix-conflict-detection
```

### Commit Messages

Write clear, professional commit messages following the [Conventional Commits](https://www.conventionalcommits.org/) specification:

- `feat:` - New feature
- `fix:` - Bug fix
- `refactor:` - Code refactoring
- `docs:` - Documentation changes
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

Example:
```
feat: add station filtering by line

- Add dropdown to filter stations by selected line
- Update graph display to show only filtered stations
- Preserve filter state in local storage
```

### Pull Requests

1. **Ensure tests pass and code is clean**
   - Run `cargo test`
   - Run `cargo clippy --all-targets -- -D warnings`

2. **Push your changes**

```bash
git push origin yourname/123-your-branch-name
```

3. **Open a Pull Request**
   - Go to the original repository on GitHub
   - Click "New Pull Request"
   - Select your fork and branch
   - Fill out the PR template with a clear description

4. **Respond to feedback**
   - Address any review comments
   - Push additional commits to your branch as needed

## Project Structure

```
nimby_graph/
├── src/
│   ├── components/     # Leptos UI components
│   ├── models/         # Data models and business logic
│   ├── storage/        # IndexedDB persistence
│   ├── import/         # CSV import functionality
│   ├── conflict.rs     # Conflict detection algorithms
│   └── lib.rs         # Main library entry point
├── style/             # SCSS stylesheets
├── terraform/         # AWS infrastructure
└── .github/           # GitHub workflows and templates
```

## Reporting Issues

- Use the bug report template for bugs
- Use the feature request template for new features
- Include as much detail as possible
- Export and share project files if the issue is data-specific

## Questions?

If you have questions about contributing, feel free to:
- Open a discussion on GitHub
- Ask in an issue
- Reach out to the maintainers

Thank you for contributing to RailGraph!
