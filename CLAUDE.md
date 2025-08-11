# cuenv Project Instructions

## Project Type

- direnv alternative using CUE for type-safe environment configuration
- Go-Rust FFI hybrid (Go for CUE evaluation, Rust main application)

## Critical Rules

- NEVER use #[allow] attributes - fix all warnings
- Zero-warnings policy - all Clippy warnings are errors
- Use only Nix-provided tools (nix develop environment)
- Version format: 0.4.4 (no 'v' prefix)
- Conventional Commits format required

## Assistance

- Whenever I ask you for a second opinion, or to speak with Gemini, use the gemini CLI:
  - gemini -p "PROMPT"

## Code Style

- Prefer immutability: use `let` over `let mut`
- Use iterators over manual loops
- Pure functions for business logic
- Many small files over large monolithic ones
- Group by feature, not type (e.g., src/state/ not models.rs)
- Use directories for modularity (my_feature/utils.rs not my_feature_utils.rs)
- Single responsibility per function/module

## Development Workflow

- Mandatory TDD: Red-Green-Refactor cycle
- ALL commands must be prefixed with nix develop -c "CMD"
- Run before commit: treefmt → cargo clippy --fix --all-targets --all-features --allow-dirty → cargo clippy --all-targets --all-features -- -D warnings → nix flake check
- Test with: cargo nextest run
- Test CI profile: cargo nextest run --profile ci
- Test coverage: cargo llvm-cov nextest --lcov --output-path lcov.info
- Test examples: scripts/test-examples.sh

## Build Commands

- cargo build (debug)
- cargo build --release
- cargo run -- <args>

## Architecture

- libcue-bridge/: Go-Rust FFI bridge for CUE evaluation
- src/env_manager.rs: Central orchestrator
- src/state.rs: Transactional state system
- src/landlock/: Linux security sandbox
- src/platform/: OS-specific abstractions

## FFI Rules

- Use CString for Rust→Go strings
- Free C strings from Go using CStr::from_ptr
- RAII patterns for FFI resource management

## Testing

- Unit tests: alongside code
- Integration tests: tests/ directory
- Property-based tests: for critical algorithms
- Example tests: examples/ directory (do not create new test directories)

## Common Tasks

- Modify CUE Schema: edit cue/schema.cue → update src/cue_config.rs → add tests in examples/
- Add Shell: implement Shell trait in src/shell/<name>.rs → add detection in src/shell_hook.rs → add test in examples/
- Update Dependencies: Rust: cargo update, Go: go mod vendor in libcue-bridge/, Nix: nix flake update

## Release Process

1. Update version in Cargo.toml
2. nix develop -c cargo update -p cuenv
3. git add Cargo.toml Cargo.lock && git commit -m "release: X.Y.Z"
4. git tag -a X.Y.Z -m "Release X.Y.Z" (no 'v' prefix)
5. git push origin main && git push origin X.Y.Z

## Requirements

- CGO required (CGO_ENABLED=1)
- Nix flake for all dependencies
- Nix flake warning "attribute 'package' missing" can be ignored
