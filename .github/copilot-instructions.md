# cuenv Development Instructions

**ALWAYS follow these instructions first. Only fallback to additional search and context gathering if the information here is incomplete or found to be in error.**

cuenv is a direnv alternative using CUE (Configure, Unify, Execute) files for type-safe environment configuration. It's a Rust workspace with Go FFI bridge for CUE evaluation, built entirely with Nix.

## Critical Setup & Build Information

### Essential Development Environment

**NEVER CANCEL: Initial `nix develop` setup takes 15-20 minutes on first run (VALIDATED)**
- Enter development environment: `nix develop` (Set timeout to 30+ minutes, NEVER CANCEL)
- Downloads ~4GB of dependencies on first run (VALIDATED: 3853.2 MiB seen in testing)
- All development must happen within the nix development shell
- Exit and re-enter with `exit` then `nix develop` if needed

### Core Build Commands

**NEVER CANCEL: Build operations take significant time (VALIDATED)**
1. `nix build` - Build the entire project (Set timeout to 60+ minutes, NEVER CANCEL - VALIDATED: 5+ minutes ongoing)
2. `nix flake check` - Run all checks including build, tests, and linting (Set timeout to 90+ minutes, NEVER CANCEL)

### Required Pre-Commit Workflow
**Run these commands in exact order before every commit:**
1. `treefmt` - Format all code (30 seconds)
2. `cargo clippy --fix --all-targets --all-features --allow-dirty` - Auto-fix linting (2-3 minutes)
3. `cargo clippy --all-targets --all-features -- -D warnings` - Check for warnings as errors (2-3 minutes)
4. `nix flake check` - Final validation (Set timeout to 90+ minutes, NEVER CANCEL)

**Zero-warnings policy: All Clippy warnings are treated as errors**

## Testing Commands

**NEVER CANCEL: Test operations take significant time**
- `cargo nextest run` - Run main test suite (Set timeout to 45+ minutes, NEVER CANCEL)
- `cargo nextest run --profile ci` - Run CI profile tests (Set timeout to 60+ minutes, NEVER CANCEL)
- `cargo test --test test_examples` - Test example configurations (Set timeout to 30+ minutes, NEVER CANCEL)
- `cargo llvm-cov nextest --lcov --output-path lcov.info` - Generate coverage (Set timeout to 60+ minutes, NEVER CANCEL)

## Project Structure

### Workspace Layout
- **crates/** - Rust workspace with 12 crates:
  - `cli/` - Main binary and CLI interface
  - `core/` - Core types, errors, and event system
  - `config/` - CUE configuration parsing
  - `env/` - Environment management
  - `task/` - Task execution engine
  - `cache/` - Caching system
  - `security/` - Security features and validation
  - `shell/` - Shell integrations (bash, zsh, fish)
  - `tui/` - Terminal UI components
  - `hooks/` - Hook management
  - `utils/` - Shared utilities
  - `libcue-ffi-bridge/` - Go/CUE FFI bridge (critical component)

### Key Files & Directories
- `env.cue` - Project's own cuenv configuration with all tasks
- `flake.nix` - Nix flake configuration for build environment
- `examples/` - 20+ example configurations for testing
- `tests/` - Integration, unit, and BDD tests
- `.config/nextest.toml` - Test runner configuration

## Validation Scenarios

**ALWAYS test functionality after making changes:**

### Basic Functionality Test
```bash
# 1. Build the project
nix build

# 2. Copy binary for testing
mkdir -p target/debug
cp result/bin/cuenv target/debug/

# 3. Test basic commands
./target/debug/cuenv --help
./target/debug/cuenv init
./target/debug/cuenv env status
```

### Example Configuration Test
```bash
# Test against example configurations
cd examples/basic
../../target/debug/cuenv allow .
../../target/debug/cuenv env status
../../target/debug/cuenv task list
```

### Complete Development Workflow Test
```bash
# 1. Enter nix environment (15-20 minutes first time)
nix develop

# 2. Format and fix code
treefmt
cargo clippy --fix --all-targets --all-features --allow-dirty

# 3. Run main tests (45+ minutes, NEVER CANCEL)
cargo nextest run

# 4. Test examples (30+ minutes, NEVER CANCEL)
cargo test --test test_examples

# 5. Final validation (90+ minutes, NEVER CANCEL)
nix flake check
```

## Development Guidelines

### Code Style & Rules
- **Zero-warnings policy**: Never use `#[allow]` attributes
- **TDD workflow**: Red-Green-Refactor cycle
- **File organization**: Many small files over monolithic ones
- **Module structure**: Group by feature, not type
- **Immutability**: Prefer `let` over `let mut`
- **Version format**: X.Y.Z (no 'v' prefix)

### Architecture Notes
- **Go FFI Bridge**: `crates/libcue-ffi-bridge/` handles CUE evaluation
- **Security**: Landlock sandbox for Linux, capabilities-based access control
- **Task System**: DAG-based execution with caching and dependencies
- **Shell Integration**: Hooks for bash, zsh, fish with auto-detection

### Common Task Patterns
- **Add shell support**: Implement Shell trait in `src/shell/<name>.rs` → add detection → test in examples
- **Modify CUE schema**: Edit schema files → update config parsing → add tests in examples
- **Update dependencies**: Rust: `cargo update`, Go: `cd crates/libcue-ffi-bridge && go mod vendor`, Nix: `nix flake update`

## Environment Variables
- `CUENV_ENV=development` (or `ci`, `production`)
- `CARGO_TERM_COLOR=always`
- `RUST_BACKTRACE=1`
- `CGO_ENABLED=1` (required for Go FFI)

## Known Issues & Workarounds
- **Nix warning** "attribute 'package' missing" can be ignored
- **First-time setup** takes 15-20 minutes due to large dependency downloads
- **Integration tests** disabled in nix environment (use scripts for manual testing)
- **Build artifacts** are cached between runs to speed up subsequent builds

## Quick Reference Commands
```bash
# Essential workflow
nix develop                    # Enter dev environment (15-20 min first time)
nix build                      # Build project (60+ min, NEVER CANCEL)
cargo nextest run             # Run tests (45+ min, NEVER CANCEL) 
treefmt                        # Format code
cargo clippy --fix --all-targets --all-features --allow-dirty  # Fix linting
nix flake check               # Final validation (90+ min, NEVER CANCEL)

# Quick debugging
cargo run -- --help          # Run cuenv CLI
cargo test --lib             # Unit tests only
cargo check                   # Fast syntax check
```

**Remember: All build and test operations take significant time. Always set appropriate timeouts (60-90+ minutes) and NEVER CANCEL long-running operations.**
