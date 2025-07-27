# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

All commands must be run within the Nix development shell:

```bash
# Building
nix develop -c cargo build                    # Build debug version
nix develop -c cargo build --release          # Build release version
nix develop -c cargo watch                    # Watch and rebuild on changes

# Testing
nix develop -c cargo test                     # Run all tests
nix develop -c cargo test <test_name>         # Run specific test
nix develop -c cargo test --lib               # Run unit tests only
nix develop -c cargo test --test <test_file>  # Run specific integration test
nix develop -c ./scripts/test-examples.sh     # Test all examples

# Code Quality
nix develop -c cargo fmt                      # Format code
nix develop -c cargo clippy                   # Run linter
nix develop -c treefmt                        # Format all code (Rust, Go, Nix, etc.)
nix develop -c nix flake check                # Check code formatting

# Running
nix develop -c cargo run -- <args>            # Run cuenv with arguments
nix develop -c cargo run --bin remote_cache_server  # Start remote cache server

# Documentation
nix develop -c cargo doc --open               # Generate and open API docs
```

## Architecture Overview

cuenv is a direnv alternative that uses CUE (Configure, Unify, Execute) files for type-safe environment configuration.

### Key Components

1. **CUE Integration**: Go-Rust FFI bridge in `libcue-bridge/` that evaluates CUE files and returns JSON
2. **Environment Manager**: Central orchestrator (`src/env_manager.rs`) that loads CUE configs, manages state, and applies environment changes
3. **State Management**: Transactional state system (`src/state.rs`) with atomic operations and rollback support
4. **Shell Integration**: Multi-shell support with auto-loading hooks for bash, zsh, fish, and others
5. **Security Layer**: Landlock-based sandboxing for disk/network access restrictions
6. **Task System**: Dependency-aware task executor with caching and parallel execution

### Important Patterns

- **Platform Abstraction**: Unix/Windows implementations behind trait (`src/platform/`)
- **RAII Resource Management**: Automatic cleanup for FFI strings, processes, and locks
- **Error Recovery**: All errors include helpful suggestions (`src/error.rs`)
- **Atomic Operations**: File writes and state changes use temporary files with atomic moves

### CUE Schema

Environment configurations are defined in `env.cue` files following the schema in `cue/schema.cue`:

- Environment variables with metadata (description, sensitive flags)
- Multiple environments (dev, staging, prod)
- Capability-based filtering
- Tasks with dependencies and caching
- Pre/post hooks
- Security restrictions (file/network access)

### Testing Strategy

- Unit tests alongside code files
- Integration tests in `tests/` covering major workflows
- Property-based tests for critical components
- Security/Landlock tests require specific kernel support
- Example-based tests via `scripts/test-examples.sh`

### Common Development Tasks

When modifying the CUE bridge:

1. Edit Go code in `libcue-bridge/`
2. The build.rs will automatically recompile during `cargo build`

When adding new shell support:

1. Implement the `Shell` trait in `src/shell/`
2. Add integration in `src/shell_hook.rs`
3. Update shell detection logic

When working on security features:

1. Landlock code is in `src/landlock/`
2. Test with `cargo test --test landlock_tests` (requires Linux with Landlock support)
3. Use audit mode (`--audit`) to trace access patterns
