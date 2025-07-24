# Contributing to cuenv

Thank you for your interest in contributing to cuenv! This document provides guidelines and instructions for contributing.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct:

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Show empathy towards other contributors

## How to Contribute

### Reporting Issues

- Check if the issue already exists
- Include a clear description of the problem
- Provide steps to reproduce the issue
- Include your environment details (OS, Rust version, etc.)

### Submitting Pull Requests

1. Fork the repository
1. Create a feature branch (`git checkout -b feature/amazing-feature`)
1. Make your changes
1. Add tests for new functionality
1. Ensure all tests pass (`cargo test`)
1. Format your code (`cargo fmt`)
1. Run clippy (`cargo clippy`)
1. Commit your changes with a descriptive message
1. Push to your fork
1. Open a Pull Request

### Development Setup

1. Install Rust (latest stable)
1. Install Go (for libcue bindings)
1. Clone the repository
1. Build the project:
   ```bash
   cargo build
   ```
1. Run tests:
   ```bash
   cargo test
   ```

### Testing

- Write unit tests for new functionality
- Add integration tests when appropriate
- Ensure test coverage doesn't decrease
- Run `cargo tarpaulin` to check coverage

### Code Style

- Follow Rust naming conventions
- Use `cargo fmt` to format code
- Run `cargo clippy` and address any warnings
- Write clear, self-documenting code
- Add comments for complex logic
- Update documentation when changing functionality

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in present tense (e.g., "Add", "Fix", "Update")
- Keep the first line under 72 characters
- Reference issues when applicable

Example:

```
Add support for recursive environment loading

- Implement directory traversal logic
- Add tests for nested env.cue files
- Update documentation

Fixes #123
```

### Documentation

- Update README.md for user-facing changes
- Add inline documentation for public APIs
- Include examples for new features
- Keep documentation concise and clear

## Questions?

Feel free to open an issue for any questions about contributing!
