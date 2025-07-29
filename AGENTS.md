# AGENTS.md

This file provides guidance to AI agents (Claude, GitHub Copilot, etc.) when working with the cuenv codebase.

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

## Architecture

### Key Components

1. **Go-Rust FFI Bridge** (`libcue-bridge/`)
   - Go code that evaluates CUE files and returns JSON
   - Built as a static C archive during `cargo build`
   - Uses CGO for C interoperability

2. **Build Script** (`build.rs`)
   - Compiles the Go bridge with `CGO_ENABLED=1`
   - Handles protobuf compilation
   - Automatically runs during `cargo build`

3. **Static Binary Support**
   - Regular build: Dynamic linking
   - Static build: Available via `nix build .#cuenv-static` (Linux only)
   - Uses musl libc for full static linking

## Development Commands

All commands should be run within the Nix development shell:

```bash
# Enter development environment
nix develop

# Building
cargo build                    # Debug build
cargo build --release          # Release build
nix build .#cuenv             # Nix build (all platforms)
nix build .#cuenv-static      # Static build (Linux only)

# Testing
cargo test                     # Run all tests
cargo nextest run             # Better test runner
./scripts/test-examples.sh    # Test example CUE files

# Code Quality
cargo fmt                     # Format Rust code
cargo clippy                  # Lint Rust code
treefmt                       # Format all code (Rust, Go, Nix, YAML)
nix flake check              # Comprehensive checks

# Running
cargo run -- init            # Initialize in current directory
cargo run -- reload          # Reload environment
```

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
     - x86_64-unknown-linux-musl
     - aarch64-apple-darwin
     - x86_64-linux-static (via Nix, includes both cuenv and remote_cache_server)

## Important Patterns

### FFI Memory Management
- Always use `CString` for passing strings to Go
- Free returned C strings with `CStr::from_ptr` and proper cleanup
- The build script handles vendored Go dependencies

### Error Handling
- All errors include helpful suggestions via the custom error type
- Use `miette` for pretty error reporting with source locations

### Platform Differences
- Linux: Full static builds available via Nix
- macOS: Dynamic linking only (no glibc.static)
- Windows: Basic support through platform abstraction layer

### Static Builds
The static build (`cuenv-static`) is only available on Linux because:
- It requires `glibc.static` or `musl` for static linking
- Uses `CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl"`
- Includes both `cuenv` and `remote_cache_server` binaries

## Testing Approach

1. **Unit Tests**: Alongside code files
2. **Integration Tests**: In `tests/` directory
3. **Example Tests**: Via `scripts/test-examples.sh`
4. **Nix Checks**: `nix flake check` runs formatting, clippy, and tests

## Common Tasks

### Adding a New Shell
1. Implement the `Shell` trait in `src/shell/<shell_name>.rs`
2. Add shell detection in `src/shell_hook.rs`
3. Update documentation

### Modifying CUE Schema
1. Edit `cue/schema.cue`
2. Update corresponding Rust types in `src/cue_config.rs`
3. Add tests for new fields

### Updating Dependencies
1. For Rust: Update `Cargo.toml` and run `cargo update`
2. For Go: Update `libcue-bridge/go.mod` and run `go mod vendor`
3. For Nix: Run `nix flake update`

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