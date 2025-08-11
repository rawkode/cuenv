# Changelog

## [0.1.1] - 2024-12-XX

### Fixed

- Updated CLI adapter to use correct `cuenv env export` command syntax instead of legacy `cuenv export`
- Improved compatibility with cuenv's new centralized configuration architecture

### Technical Improvements

- CLI adapter now uses the optimized command structure that benefits from Arc<Config> sharing
- Faster environment variable loading through improved command execution
- Compatible with cuenv's new subcommand structure (task/env subcommands)

## [0.1.0] - Initial Release

### Features

- Environment variable tree view with masking support
- Task tree view with execution capabilities
- Auto-loading of cuenv environments
- Terminal integration for task execution
- Context menu actions for copying environment variables
- File watching for automatic reloading

### Commands

- `cuenv.reload` - Reload environment manually
- `cuenv.runTask` - Execute tasks from task panel
- `cuenv.toggleMasking` - Toggle sensitive variable masking
- Various copy and terminal commands

### Configuration

- Configurable cuenv executable path
- Auto-load settings
- Mask patterns for sensitive variables
- Terminal strategy configuration
