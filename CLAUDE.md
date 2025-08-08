# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) and other AI agents when working with the cuenv codebase.

## Project Overview

cuenv is a direnv alternative that uses CUE (Configure, Unify, Execute) files for type-safe environment configuration. It integrates Go (for CUE evaluation) with Rust (for the main application) through FFI.

## Versioning Convention

**IMPORTANT**: This project uses semantic versioning WITHOUT the 'v' prefix:

- ✅ Correct: `0.4.4`
- ❌ Incorrect: `v0.4.4`

This applies to:

- Git tags (e.g., `git tag 0.4.4`)
- GitHub releases
- Cargo.toml version field

## Development Commands

All commands should be run within the Nix development shell:

```bash
# Enter development environment
nix develop

# Building
nix develop -c cargo build                    # Build debug version
nix develop -c cargo build --release          # Build release version
nix develop -c cargo watch                    # Watch and rebuild on changes
nix build .#cuenv                            # Nix build (all platforms)

# Testing
nix develop -c cargo test                     # Run all tests
nix develop -c cargo test <test_name>         # Run specific test
nix develop -c cargo test --lib               # Run unit tests only
nix develop -c cargo test --test <test_file>  # Run specific integration test
nix develop -c ./scripts/test-examples.sh     # Test all examples

# Testing with Nextest (faster, better output)
nix develop -c cargo nextest run              # Run all tests with nextest
nix develop -c cargo nextest run <test_name>  # Run specific test
nix develop -c cargo nextest run --profile ci # Run tests with CI profile
nix develop -c cargo nextest run --profile quick # Quick test run for development
nix develop -c cargo nextest list             # List all tests without running

# Test Coverage with Nextest
nix develop -c cargo llvm-cov nextest         # Generate test coverage
nix develop -c cargo llvm-cov nextest --lcov --output-path lcov.info # Generate lcov report

# Code Quality
nix develop -c cargo fmt                      # Format Rust code
nix develop -c cargo clippy                   # Lint Rust code
nix develop -c treefmt                        # Format all code (Rust, Go, Nix, YAML)
nix develop -c nix flake check                # Comprehensive checks

# Running
nix develop -c cargo run -- <args>            # Run cuenv with arguments
nix develop -c cargo run -- init              # Initialize in current directory
nix develop -c cargo run -- reload            # Reload environment

# Documentation
nix develop -c cargo doc --open               # Generate and open API docs
```

## Architecture Overview

cuenv is a direnv alternative that uses CUE (Configure, Unify, Execute) files for type-safe environment configuration.

### Key Components

1. **Go-Rust FFI Bridge** (`libcue-bridge/`)
   - Go code that evaluates CUE files and returns JSON
   - Built as a static C archive during `cargo build`
   - Uses CGO for C interoperability

2. **Build Script** (`build.rs`)
   - Compiles the Go bridge with `CGO_ENABLED=1`
   - Handles protobuf compilation
   - Automatically runs during `cargo build`

3. **Environment Manager** (`src/env_manager.rs`)
   - Central orchestrator that loads CUE configs, manages state, and applies environment changes

4. **State Management** (`src/state.rs`)
   - Transactional state system with atomic operations and rollback support

5. **Shell Integration**
   - Multi-shell support with auto-loading hooks for bash, zsh, fish, and others

6. **Security Layer**
   - Landlock-based sandboxing for disk/network access restrictions

7. **Task System**
   - Dependency-aware task executor with caching and parallel execution

### Binary Support

- Regular build: Dynamic linking
- Standard platform-specific builds via Cargo

### Important Patterns

- **Platform Abstraction**: Unix/Windows implementations behind trait (`src/platform/`)
- **RAII Resource Management**: Automatic cleanup for FFI strings, processes, and locks
- **Error Recovery**: All errors include helpful suggestions via custom error type (`src/error.rs`)
- **Atomic Operations**: File writes and state changes use temporary files with atomic moves
- **Error Handling**: Use `miette` for pretty error reporting with source locations

### FFI Memory Management

- Always use `CString` for passing strings to Go
- Free returned C strings with `CStr::from_ptr` and proper cleanup
- The build script handles vendored Go dependencies

### Platform Differences

- Linux: Standard glibc builds
- macOS: Dynamic linking with system frameworks
- Windows: Basic support through platform abstraction layer

### CUE Schema

Environment configurations are defined in `env.cue` files following the schema in `cue/schema.cue`:

- Environment variables with metadata (description, sensitive flags)
- Multiple environments (dev, staging, prod)
- Capability-based filtering
- Tasks with dependencies and caching
- Pre/post hooks
- Security restrictions (file/network access)

### Testing Strategy

1. **Unit Tests**: Alongside code files
2. **Integration Tests**: In `tests/` directory covering major workflows
3. **Property-based Tests**: For critical components
4. **Security/Landlock Tests**: Require specific kernel support
5. **Example Tests**: Via `scripts/test-examples.sh`
6. **Nix Checks**: `nix flake check` runs formatting, clippy, and tests

### Common Development Tasks

#### Modifying the CUE Bridge

1. Edit Go code in `libcue-bridge/`
2. The build.rs will automatically recompile during `cargo build`

#### Adding a New Shell

1. Implement the `Shell` trait in `src/shell/<shell_name>.rs`
2. Add shell detection in `src/shell_hook.rs`
3. Update documentation

#### Modifying CUE Schema

1. Edit `cue/schema.cue`
2. Update corresponding Rust types in `src/cue_config.rs`
3. Add tests for new fields

#### Working on Security Features

1. Landlock code is in `src/landlock/`
2. Test with `cargo test --test landlock_tests` (requires Linux with Landlock support)
3. Use audit mode (`--audit`) to trace access patterns

#### Updating Dependencies

1. For Rust: Update `Cargo.toml` and run `cargo update`
2. For Go: Update `libcue-bridge/go.mod` and run `go mod vendor`
3. For Nix: Run `nix flake update`

## Release Process

1. **Update Version**

   ```bash
   # Edit Cargo.toml
   version = "0.4.5"  # No 'v' prefix!

   # Update Cargo.lock
   nix develop -c cargo update -p cuenv
   ```

2. **Commit Changes**

   ```bash
   git add Cargo.toml Cargo.lock
   git commit -m "release: 0.4.5"
   ```

3. **Create Tag** (NO 'v' prefix!)

   ```bash
   git tag -a 0.4.5 -m "Release 0.4.5"
   git push origin main
   git push origin 0.4.5
   ```

4. **GitHub Release**
   - Automatically triggered by tag push
   - Creates binaries for:
     - x86_64-unknown-linux-gnu
     - aarch64-apple-darwin

## Gotchas

1. **CGO is Required**: The project cannot be built with `CGO_ENABLED=0`
2. **Protobuf**: Required for building (remote cache features)
3. **Go Version**: Requires Go 1.24+ for CUE support
4. **Vendored Dependencies**: Go dependencies are vendored via Nix

## Security Considerations

- Environment variables can be marked as `sensitive`
- Landlock support for sandboxing (Linux only)
- Audit mode available for permission tracking
- Never commit secrets to `.envrc` or `env.cue` files

## Commit Message Format

Follow conventional commits:

- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation changes
- `chore:` Maintenance tasks
- `release:` Version updates (e.g., `release: 0.4.5`)

Remember: NO 'v' prefix in version numbers!
