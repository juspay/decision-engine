# Contributing to Decision Engine

Thank you for considering contributing to Decision Engine! This document describes how to report issues, propose changes, and submit pull requests.

## Code of Conduct

Be respectful, constructive, and professional. We follow the spirit of the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). Unacceptable behaviour can be reported privately to `hyperswitch@juspay.in`.

## How Can I Contribute?

### Reporting Bugs

Bug reports help make Decision Engine more stable. When you submit a bug report, please include:

1. **Version Information**: Include the Decision Engine version you're using
2. **Environment**: Include OS, Rust version, Redis version, etc.
3. **Steps to Reproduce**: Detailed steps to reproduce the bug
4. **Expected vs. Actual Behavior**: What you expected vs. what happened
5. **Configuration**: Your configuration settings (redact sensitive info)

Please use the issue template when submitting a bug report.

### Suggesting Features

We welcome feature suggestions! Feature requests should include:

1. **Clear Description**: Describe the feature and its benefits
2. **Use Case**: Explain how it would be used
3. **Alternatives**: Any alternative solutions you've considered

### Contributing Code

1. **Fork the Repository**: Fork the project to your GitHub account
2. **Create a Branch**: Create a feature branch for your work
3. **Write Code**: Write your code, following our code style
4. **Write Tests**: Add tests for your changes
5. **Submit a Pull Request**: Submit a PR against the main branch

#### Development Environment Setup

The canonical local setup — CLI, Docker, Compose, and Helm — lives in [docs/local-setup.md](docs/local-setup.md). Database-specific quickstarts:

- **PostgreSQL**: [docs/setup-guide-postgres.md](docs/setup-guide-postgres.md)
- **MySQL**: [docs/setup-guide-mysql.md](docs/setup-guide-mysql.md)

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/decision-engine.git
cd decision-engine

# Track upstream
git remote add upstream https://github.com/juspay/decision-engine.git

# Create a branch
git checkout -b feat/your-feature-name

# Recommended dev tooling
cargo install cargo-watch cargo-insta cargo-audit

# Run the test suite
cargo test

# Auto-reload loop during development
cargo watch -x run
```

## Code Style

We follow Rust's official style guidelines:

- Run `cargo fmt` before committing
- Run `cargo clippy` to catch common mistakes
- Aim for clear, idiomatic Rust code
- Document your public API with doc comments

## Commit Messages

- Use the imperative mood ("Add feature" not "Added feature")
- First line is a summary (50 chars or less)
- Include relevant issue numbers (e.g., "Fix #42: Add success rate validation")
- Be descriptive about what and why, not how

## Pull Request Process

1. Ensure your code passes all tests
2. Update documentation if needed
3. Update the CHANGELOG.md if appropriate
4. Ensure the PR description clearly describes the problem and solution
5. Include any issue numbers in the PR description

Your PR will be reviewed by maintainers who may request changes.

## Testing

- Write unit tests for new functionality
- Ensure all tests pass before submitting PRs
- Consider adding integration tests for complex features

## Documentation

- Update documentation for new features or changes
- Write clear, concise doc comments for public APIs
- Update examples if appropriate

## Release Process

Maintainers will handle the release process, which includes:

1. Updating version numbers
2. Creating changelog entries
3. Creating GitHub releases
4. Publishing to registries

## Getting Help

If you need help, you can:

- Open a discussion on GitHub
- Ask in our community chat
- Check the documentation

## Recognition

All contributors will be recognized in our README and CONTRIBUTORS file.

Thank you for contributing to Decision Engine!
