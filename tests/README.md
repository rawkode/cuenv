# cuenv Test Infrastructure

## Overview

The cuenv test suite is organized into several categories to ensure comprehensive coverage of all functionality. Tests are written in Rust using various testing frameworks and follow Test-Driven Development (TDD) principles.

## Test Organization

```
tests/
├── behaviours/           # BDD scenarios using cucumber-rs
│   ├── features/        # Gherkin feature files
│   ├── steps/           # Step definitions
│   └── world.rs         # Test context/state
├── examples/            # CUE deserialization tests for all examples
├── shells/              # Shell-specific integration tests
├── performance/         # Performance benchmarks and regression tests
├── snapshots/           # CLI output consistency tests
├── contracts/           # FFI bridge and protocol contract tests
├── helpers/             # Shared test utilities and fixtures
└── mod.rs              # Test module organization
```

## Test Categories

### 1. Unit Tests (In-File)

Unit tests are located alongside the code they test, following the project standard of keeping tests in the same file as the implementation.

```rust
// In src/some_module.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        // Test implementation
    }
}
```

### 2. BDD Tests (tests/behaviours/)

Behavior-Driven Development tests using cucumber-rs framework. These tests describe high-level functionality in Gherkin syntax.

**Features:**

- `shell_integration.feature` - Shell detection, hook installation, environment activation
- `environment_lifecycle.feature` - Loading, switching, unloading environments
- `hook_execution.feature` - Pre/post hooks, background hooks, hook chains
- `secret_resolution.feature` - Secret resolver protocols, caching, security
- `task_execution.feature` - Task discovery, execution, groups
- `supervisor_lifecycle.feature` - Supervisor management of long-running processes

### 3. Shell Integration Tests (tests/shells/)

Tests for shell-specific functionality using expectrl for PTY simulation.

- Bash integration
- Zsh integration
- Fish integration
- Nushell integration
- Interactive mode testing

### 4. Example Tests (tests/examples/)

Automated tests for all examples in the `examples/` directory:

- CUE deserialization
- Environment selection
- Capability tags
- Secret resolution
- Hook execution

### 5. Performance Tests

Benchmarks and performance regression tests using criterion.

### 6. Property-Based Tests

Using proptest for testing edge cases and invariants.

## Running Tests

### All Tests

```bash
nix develop -c cargo nextest run
```

### Specific Test Categories

#### BDD Tests

```bash
nix develop -c cargo test --test behaviours
```

#### Shell Integration Tests

```bash
nix develop -c cargo test --test shells
```

#### Example Tests

```bash
nix develop -c cargo test --test examples
```

#### Unit Tests Only

```bash
nix develop -c cargo nextest run --lib
```

#### Integration Tests Only

```bash
nix develop -c cargo nextest run --tests
```

### With Coverage

```bash
nix develop -c cargo llvm-cov nextest --lcov --output-path lcov.info
```

### CI Profile

```bash
nix develop -c cargo nextest run --profile ci
```

## Testing Dependencies

All testing dependencies are managed through `Cargo.toml`:

- **cucumber** (0.21) - BDD framework
- **insta** (1.40) - Snapshot testing
- **criterion** (0.5) - Benchmarking
- **proptest** (1.5) - Property-based testing
- **expectrl** (0.7) - PTY testing for interactive mode
- **serial_test** (3.1) - Serialized test execution
- **rstest** (0.23) - Parametrized testing

## Shell Availability

All shells required for testing are provided through Nix:

- bash
- zsh
- fish
- nushell
- elvish
- dash

These are automatically available in the `nix develop` environment.

## Writing Tests

### Guidelines

1. **Unit Tests**: Keep in the same file as the code being tested
2. **Integration Tests**: Use appropriate subdirectory in `tests/`
3. **BDD Tests**: Write feature files first, then implement steps
4. **Property Tests**: Focus on invariants and edge cases
5. **Performance Tests**: Include baseline measurements

### Test Naming Conventions

- Unit tests: `test_<function_name>_<scenario>`
- Integration tests: `test_<feature>_<scenario>`
- BDD features: `<feature>.feature`
- Property tests: `prop_<property_being_tested>`

### Test Data

- Use `tempfile::TempDir` for isolated test environments
- Place static test data in `tests/fixtures/`
- Generate dynamic test data within tests

## CI Integration

Tests run automatically on:

- Every push to main
- Pull requests
- Nightly builds (full test suite including performance)

GitHub Actions workflow uses Nix to ensure consistent shell availability across all platforms.

## Troubleshooting

### Common Issues

1. **Shell not found**: Ensure you're running tests within `nix develop`
2. **Binary not found**: Build cuenv first with `cargo build`
3. **Flaky tests**: Check for race conditions, use `serial_test` if needed
4. **PTY tests failing**: May need to run with proper terminal allocation

### Debug Output

Enable debug output for tests:

```bash
RUST_LOG=debug cargo test -- --nocapture
```

## Contributing

When adding new functionality:

1. Write BDD scenarios for user-facing behavior
2. Add unit tests for implementation details
3. Include integration tests for cross-boundary interactions
4. Add property tests for complex algorithms
5. Update this README if adding new test categories
