# Contributor Guide: cuenv

Welcome to the `cuenv` project! This guide provides everything you need to know to contribute effectively. Reading and understanding this document is the first and most important step.

## 1. Project Overview

`cuenv` is a `direnv` alternative that uses CUE (Configure, Unify, Execute) for type-safe environment configuration. It provides a robust way to manage complex development environments with the safety and expressiveness of the CUE language.

The project integrates a Go core (for CUE evaluation) with a Rust main application through a Foreign Function Interface (FFI), combining the strengths of both ecosystems.

## 2. Core Philosophy & Principles

To ensure the long-term health and maintainability of the codebase, we adhere to a specific set of principles. All contributions must align with this philosophy.

### Modularity and High Cohesion

The primary goal is long-term maintainability. We achieve this by writing code that is simple, composable, and easy to understand in isolation.

- **Many Small Units**: Prefer many small, focused files, functions, and modules over large, monolithic ones. If a file or function has too many responsibilities, break it apart.
- **Group by Feature, Not Type**: Code that changes together should live together. Instead of organizing files by their type (e.g., `models.rs`, `errors.rs`), organize them by the feature they implement (e.g., a `src/state/` module containing all logic, tests, and errors related to state management). This is a critical pattern in this codebase.
- **Single Responsibility**: Every function and module should have a single, well-defined responsibility.
- **Use Directories for Modularity**: Organize code into a clear directory hierarchy. **Do not use underscores in filenames (e.g., `my_feature_utils.rs`) as a substitute for creating a dedicated module directory (e.g., `my_feature/utils.rs`).** A clean directory structure is essential for discoverability and long-term maintenance.

### Functional-First Approach

We favor a style inspired by functional programming to minimize side effects and improve predictability.

- **Immutability by Default**: Always use `let` instead of `let mut` unless mutability is unavoidable. Immutable data structures are easier to reason about.
- **Pure Functions for Logic**: Core business logic should be implemented as pure functions—functions whose output is determined only by their inputs, with no observable side effects.
- **Embrace Iterators**: Use iterator methods (`map`, `filter`, `fold`, etc.) over manual loops. This leads to more declarative and less error-prone code.

**Example:**

- **❌ Avoid (Imperative, Mutable):**
  ```rust
  let mut results = Vec::new();
  for item in some_list {
      if item.is_valid() {
          results.push(item.process());
      }
  }
  ```
- **✅ Prefer (Functional, Immutable):**
  ```rust
  let results: Vec<_> = some_list
      .into_iter()
      .filter(|item| item.is_valid())
      .map(|item| item.process())
      .collect();
  ```

### Test-Driven Development (TDD)

Development of any new feature or bug fix **must** follow the TDD workflow. This is non-negotiable.

The cycle is **Red-Green-Refactor**:

1.  **🔴 Red**: Write a failing test that captures the requirements.
2.  **🟢 Green**: Write the simplest possible code to make the test pass.
3.  **🔵 Refactor**: Clean up the code and the test while ensuring the suite remains green.

## 3. Critical Development Rules

These rules are enforced automatically by our CI and are not subject to debate.

- **No-Warnings Policy**: This project has a strict **zero-warnings policy**. All Clippy warnings are treated as errors and will fail the build. **NEVER use `#[allow]` attributes** to suppress warnings. You must fix the underlying issue.

- **Nix-Only Dependencies**: **Only use software provided by the Nix flake.** Do not rely on system-installed tools (including your own `rustc` or `go`). The Nix flake creates a 100% reproducible development environment. Always enter the environment first with `nix develop` or prefix commands with `nix develop -c`.

- **Versioning Convention**: This project uses semantic versioning **WITHOUT the 'v' prefix**.
  - ✅ Correct: `0.4.4`
  - ❌ Incorrect: `v0.4.4`
    This applies to Git tags, GitHub releases, and the `Cargo.toml` version field.

- **Commit Message Format**: Follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification (e.g., `feat:`, `fix:`, `docs:`, `refactor:`, `release:`).

## 4. Development Workflow & Commands

All commands must be run from within the Nix development shell.

```bash
# Enter the development environment (do this first!)
nix develop
```

### Code Quality & Formatting

Always run these commands in order before committing:

```bash
# 1. Format all code in the project (Rust, Go, Nix, etc.)
treefmt

# 2. Auto-fix simple clippy warnings
cargo clippy --fix --all-targets --all-features --allow-dirty

# 3. Manually fix any remaining warnings
cargo clippy --all-targets --all-features -- -D warnings

# 4. Run all checks (formatting, clippy, tests) - the ultimate gatekeeper
nix flake check
```

### Building & Running

```bash
# Build debug version
cargo build

# Build release version
cargo build --release

# Run the application with arguments
cargo run -- <args>
```

### Testing

We use `nextest` for a faster and more informative testing experience.

```bash
# Run all tests
cargo nextest run

# Run tests with the CI profile
cargo nextest run --profile ci

# Generate a test coverage report
cargo llvm-cov nextest --lcov --output-path lcov.info
```

## 5. Architecture Deep Dive

### Key Components

- **Go-Rust FFI Bridge** (`libcue-bridge/`): Go code compiled into a static C archive that evaluates CUE files. The `build.rs` script orchestrates this.
- **Environment Manager** (`src/env_manager.rs`): The central orchestrator for loading configs and applying changes.
- **State Management** (`src/state.rs`): A transactional state system with atomic operations.
- **Security Layer** (`src/landlock/`): A Landlock-based sandbox for restricting system access on Linux.
- **Platform Abstraction** (`src/platform/`): Traits and implementations for OS-specific differences.

### FFI Memory Management

- Always use `CString` for passing strings from Rust to Go.
- Always free C strings returned from Go using `CStr::from_ptr`.
- RAII patterns are used extensively to manage the lifetime of FFI resources automatically.

### Testing Strategy

- **Unit Tests**: Live alongside the code they test.
- **Integration Tests**: In the `tests/` directory, covering major user workflows.
- **Property-based Tests**: For critical, algorithmic components.
- **Example Tests**: `scripts/test-examples.sh` tests all configurations in `examples/`. **Do not add new test directories; use the examples.**

## 6. Common Tasks & Procedures

### Modifying the CUE Schema

1.  Edit `cue/schema.cue`.
2.  Update Rust types in `src/cue_config.rs`.
3.  Add/update tests in the `examples/` directory.

### Adding a New Shell

1.  Implement the `Shell` trait in `src/shell/<shell_name>.rs`.
2.  Add detection logic in `src/shell_hook.rs`.
3.  Provide a test case in the `examples/` directory.

### Updating Dependencies

- **Rust**: Update `Cargo.toml` and run `cargo update`.
- **Go**: Update `libcue-bridge/go.mod` and run `go mod vendor` inside that directory.
- **Nix**: Run `nix flake update`.

### Release Process

1.  **Update Version**: Change `version` in `Cargo.toml`.
2.  **Update Lockfile**: Run `nix develop -c cargo update -p cuenv`.
3.  **Commit**: `git add Cargo.toml Cargo.lock` and `git commit -m "release: 0.5.0"`.
4.  **Tag & Push**: `git tag -a 0.5.0 -m "Release 0.5.0"` (no 'v' prefix!), then `git push origin main && git push origin 0.5.0`.

## 7. Gotchas

- **CGO is Required**: Cannot be built with `CGO_ENABLED=0`.
- **Use Existing Examples**: Do not create new test directories. Add to or modify the comprehensive configurations in the `examples/` directory.
- **Nix Flake Warning**: An `"attribute 'package' missing"` warning from `nix flake check` can be safely ignored.
