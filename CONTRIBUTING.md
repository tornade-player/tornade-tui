# Contributing to tornade-tui

Thank you for your interest in contributing! This guide covers everything you need to get started.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork:
   ```bash
   git clone https://github.com/<your-username>/tornade-tui.git
   cd tornade-tui
   ```
3. **Build and test** to verify your environment:
   ```bash
   cargo build
   cargo test
   ```

## Submitting Changes

1. Create a branch for your change:
   ```bash
   git checkout -b fix/my-improvement
   ```
2. Make your changes
3. Run quality checks before committing:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test --all-features
   ```
4. Commit and push:
   ```bash
   git push origin fix/my-improvement
   ```
5. **Open a pull request** on GitHub — CI will run automatically on Linux, macOS, and Windows

## Code Style

- **Formatting**: `cargo fmt` — all code must pass `cargo fmt -- --check`
- **Linting**: `cargo clippy -- -D warnings` — no clippy warnings allowed
- **Tests**: all new functionality must include tests; run `cargo test --all-features`

## Commit Message Conventions

Use the imperative mood in the subject line:
- `Add playlist navigation panel`
- `Fix crash when library is empty`
- `Improve keybinding response time`

Keep subject lines under 72 characters. Add a body if the change needs explanation.

## Reporting Bugs

Open an issue at [github.com/tornade-player/tornade-tui/issues](https://github.com/tornade-player/tornade-tui/issues) with:
- Your operating system and Rust version (`rustc --version`)
- Steps to reproduce the issue
- Expected vs. actual behavior

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
