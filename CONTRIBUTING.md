# Contributing to AGM CLI

Thank you for your interest in contributing to AGM CLI. This document covers the development setup, testing conventions, and contribution workflow.

## Prerequisites

- **Rust 1.85+** (edition 2024)
- **Cargo** (comes with Rust)
- **Git**

## Development Setup

```bash
git clone https://github.com/JAAvila-Of/agm-cli.git
cd agm-cli
cargo build
cargo test
```

## Project Structure

```
crates/
  agm-core/    # Library: parser, validator, loader, graph, renderer
  agm-cli/     # Binary: CLI commands and runtime orchestration
docs/
  spec/        # AGM format specification
  api.md       # CLI and library API reference
tests/         # Shared test fixtures
```

## Code Quality

Before submitting a PR, ensure all checks pass:

```bash
cargo fmt --check          # Formatting
cargo clippy -- -D warnings # Linting
cargo test                  # All tests
```

## Testing Conventions

- **Unit tests**: Co-located in each module under `#[cfg(test)]`
- **Integration tests**: In `crates/*/tests/` using fixture files from `tests/fixtures/`
- **Snapshot tests**: Using `insta` for renderer output verification
- **E2E tests**: Using `assert_cmd` for CLI command testing
- **Test naming**: `test_<what>_<condition>_<expected>` (e.g., `test_parse_missing_header_returns_error`)

Every validation rule in the spec must have at least one corresponding test.

## Code Style

- No `unsafe` code
- Use `thiserror` for error types in `agm-core`, `anyhow` in `agm-cli`
- Derive `Debug`, `Clone`, `PartialEq` on all model types
- Never panic in library code; return `Result`
- See the [AGM Specification](docs/spec/agm_spec_v1.1.0.md) for format rules and design decisions

## Contribution Workflow

1. Fork the repository
2. Create a feature branch from `main`: `git checkout -b feature/my-feature`
3. Make your changes with tests
4. Ensure all checks pass (`cargo fmt`, `cargo clippy`, `cargo test`)
5. Commit with a clear message describing the change
6. Open a Pull Request against `main`

## Reporting Issues

Open an issue on [GitHub Issues](https://github.com/JAAvila-Of/agm-cli/issues) with:

- A clear description of the problem or feature request
- Steps to reproduce (for bugs)
- Expected vs actual behavior
- AGM file samples if relevant (use code blocks)

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
