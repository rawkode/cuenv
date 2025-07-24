# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5] - 2025-01-24

### Added
- Package-based CUE evaluation for better modularity and reusability
- Windows platform support with proper cross-compilation
- Nix flake for reproducible builds and development environments
- Support for 1Password section-based references in secret resolution
- Major refactoring with improved CUE integration and error handling

### Fixed
- Resolved all clippy lints for cleaner codebase
- Updated README with correct CUE syntax examples
- Proper linking of Security & CoreFoundation frameworks on macOS

### Changed
- Migrated from file-based to package-based CUE evaluation
- Improved cross-platform support and build process

## [0.1.0] - 2024-06-09

### Added
- Initial release of cuenv
- CUE file parsing for environment configuration
- Automatic environment loading when entering directories
- Hierarchical environment loading from parent directories
- Shell hooks for bash, zsh, and fish
- `cuenv run` command for hermetic environment execution
- Secret resolution from 1Password and GCP Secrets Manager
- Automatic secret obfuscation in stdout/stderr output
- Shell variable expansion support
- Type-safe configuration with CUE language features
- Support for string interpolation and computed values
- Comprehensive test suite

### Security
- Secrets are only resolved when using `cuenv run` command
- All resolved secrets are automatically obfuscated in command output
- Hermetic environment execution prevents parent environment leakage

[Unreleased]: https://github.com/rawkode/cuenv/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/rawkode/cuenv/compare/v0.1.0...v0.1.5
[0.1.0]: https://github.com/rawkode/cuenv/releases/tag/v0.1.0
