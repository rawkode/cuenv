# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/korora-tech/cuenv/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/korora-tech/cuenv/releases/tag/v0.1.0