# cuenv Codebase Refactoring Plan

## Overview

This document outlines a comprehensive plan to refactor the cuenv codebase from its current flat structure into smaller, more modular components following idiomatic Rust patterns. The goal is to improve maintainability, enhance developer experience, and establish clear separation of concerns.

## Current State Analysis

The cuenv codebase currently has a flat structure with 39 modules directly in `src/lib.rs`. While some functionality is already organized into subdirectories (`cache/`, `platform/`, `remote_cache/`, `shell/`, `tracing/`, `tui/`), many related components are scattered across the root directory, making the codebase harder to navigate and maintain.

## Proposed Modular Structure

### 1. Core Domain Modules (`src/core/`)

```
src/core/
├── mod.rs
├── types.rs          # Move from src/types.rs
├── errors.rs         # Move from src/errors.rs
├── constants.rs      # Move from src/constants.rs
├── env/
│   ├── mod.rs
│   ├── manager.rs    # Move from src/env_manager.rs
│   ├── diff.rs       # Move from src/env_diff.rs
│   └── variables.rs  # New module for env variable handling
├── state/
│   ├── mod.rs
│   ├── manager.rs    # Move from src/state.rs
│   └── transaction.rs # Extract from src/state.rs
├── config/
│   ├── mod.rs
│   ├── cue.rs        # Move from src/cue_parser.rs
│   └── loader.rs     # New module for config loading
└── hooks/
    ├── mod.rs
    └── manager.rs    # Move from src/hook_manager.rs
```

### 2. Execution Engine (`src/execution/`)

```
src/execution/
├── mod.rs
├── engine/
│   ├── mod.rs
│   ├── task.rs       # Move from src/task_executor.rs
│   └── tui.rs        # Move from src/task_executor_tui.rs
├── command/
│   ├── mod.rs
│   ├── executor.rs   # Move from src/command_executor.rs
│   └── factory.rs    # Extract from src/command_executor.rs
├── process/
│   ├── mod.rs
│   ├── spawn.rs      # New module for process spawning
│   └── monitor.rs    # New module for process monitoring
└── runtime/
    ├── mod.rs
    └── async.rs      # Move from src/async_runtime.rs
```

### 3. Security & Access Control (`src/security/`)

```
src/security/
├── mod.rs
├── validator.rs     # Move from src/security.rs
├── restrictions/
│   ├── mod.rs
│   ├── builder.rs   # Move from src/access_restrictions_builder.rs
│   └── access.rs    # Move from src/access_restrictions.rs
├── audit/
│   ├── mod.rs
│   └── logger.rs    # Move from src/audit.rs
├── secrets/
│   ├── mod.rs
│   └── manager.rs   # Move from src/secrets.rs
└── sandbox/
    ├── mod.rs
    └── landlock.rs  # New module for sandboxing
```

### 4. Cache & Storage (`src/storage/`)

```
src/storage/
├── mod.rs
├── cache/
│   ├── mod.rs       # Move from src/cache/mod.rs
│   ├── manager.rs   # Move from src/cache/cache_manager.rs
│   ├── engine.rs    # Move from src/cache/engine.rs
│   ├── config.rs    # Move from src/cache/config.rs
│   ├── item.rs      # Move from src/cache/item.rs
│   ├── mode.rs      # Move from src/cache/mode.rs
│   ├── hash.rs      # Move from src/cache/hash_engine.rs
│   └── concurrent.rs # Move from src/cache/concurrent_cache.rs
├── remote/
│   ├── mod.rs       # Move from src/remote_cache/mod.rs
│   ├── client.rs    # New module for remote cache client
│   ├── server.rs    # Move from src/remote_cache/simple_server.rs
│   └── proto.rs     # Move from src/remote_cache/grpc_proto.rs
└── file/
    ├── mod.rs
    ├── atomic.rs    # Move from src/atomic_file.rs
    ├── times.rs     # Move from src/file_times.rs
    └── directory.rs # Move from src/directory.rs
```

### 5. Platform & Shell Integration (`src/platform/` - Restructure)

```
src/platform/
├── mod.rs
├── shell/
│   ├── mod.rs       # Move from src/shell/mod.rs
│   ├── bash.rs      # Move from src/shell/bash.rs
│   ├── zsh.rs       # Move from src/shell/zsh.rs
│   ├── fish.rs      # Move from src/shell/fish.rs
│   ├── powershell.rs # Move from src/shell/pwsh.rs
│   ├── cmd.rs       # Move from src/shell/cmd.rs
│   ├── elvish.rs    # Move from src/shell/elvish.rs
│   ├── tcsh.rs      # Move from src/shell/tcsh.rs
│   ├── murex.rs     # Move from src/shell/murex.rs
│   └── hook.rs      # Move from src/shell_hook.rs
├── os/
│   ├── mod.rs
│   ├── unix.rs      # Move from src/platform/unix.rs
│   └── windows.rs   # Move from src/platform/windows.rs
└── detect.rs       # New module for platform detection
```

### 6. User Interface (`src/ui/`)

```
src/ui/
├── mod.rs
├── tui/
│   ├── mod.rs       # Move from src/tui/mod.rs
│   ├── app.rs       # Move from src/tui/app.rs
│   ├── terminal.rs  # Move from src/tui/terminal.rs
│   ├── events.rs    # Move from src/tui/events.rs
│   ├── event_bus.rs # Move from src/tui/event_bus.rs
│   ├── fallback.rs  # Move from src/tui/fallback.rs
│   └── components/
│       ├── mod.rs   # Move from src/tui/components/mod.rs
│       ├── focus.rs # Move from src/tui/components/focus_pane.rs
│       └── minimap.rs # Move from src/tui/components/minimap.rs
├── tracing/
│   ├── mod.rs       # Move from src/tracing/mod.rs
│   ├── progress.rs  # Move from src/tracing/progress.rs
│   ├── task.rs      # Move from src/tracing/task_span.rs
│   ├── tree.rs      # Move from src/tracing/tree_formatter.rs
│   └── subscriber.rs # Move from src/tracing/tree_subscriber.rs
└── output/
    ├── mod.rs
    └── filter.rs    # Move from src/output_filter.rs
```

### 7. Utilities & Support (`src/utils/`)

```
src/utils/
├── mod.rs
├── sync/
│   ├── mod.rs
│   └── env.rs       # Move from src/sync_env.rs
├── cleanup/
│   ├── mod.rs
│   └── handler.rs   # Move from src/cleanup.rs
├── network/
│   ├── mod.rs
│   ├── rate_limit.rs # Move from src/rate_limit.rs
│   └── retry.rs     # Move from src/retry.rs
├── resilience/
│   ├── mod.rs
│   └── circuit.rs   # Move from src/resilience.rs
├── memory/
│   ├── mod.rs
│   └── manager.rs   # Move from src/memory.rs
├── xdg/
│   ├── mod.rs
│   └── paths.rs     # Move from src/xdg.rs
├── compression/
│   ├── mod.rs
│   └── gzip.rs      # Move from src/gzenv.rs
└── limits/
    ├── mod.rs
    └── resources.rs # Move from src/resource_limits.rs
```

### 8. Application Entry Points

```
src/
├── lib.rs           # Simplified to re-export modules
├── main.rs          # CLI application entry point
└── bin/
    └── server.rs    # Move from src/remote_cache/bin/server.rs
```

## Module Responsibilities

### Core Domain (`src/core/`)

- **types**: Domain-specific types and zero-knowledge wrappers
- **errors**: Centralized error handling and result types
- **constants**: Application-wide constants
- **env**: Environment variable management and diffing
- **state**: Application state management and transactions
- **config**: CUE configuration parsing and loading
- **hooks**: Lifecycle hook management

### Execution Engine (`src/execution/`)

- **engine**: Task execution with dependency resolution
- **command**: Command execution and factory patterns
- **process**: Low-level process spawning and monitoring
- **runtime**: Async runtime management

### Security & Access Control (`src/security/`)

- **validator**: Input validation and security checks
- **restrictions**: Access restriction building and management
- **audit**: Audit logging and monitoring
- **secrets**: Secret resolution and management
- **sandbox**: Process sandboxing and containment

### Cache & Storage (`src/storage/`)

- **cache**: Local caching with content-addressed storage
- **remote**: Remote cache client and server functionality
- **file**: File operations and atomic updates

### Platform & Shell Integration (`src/platform/`)

- **shell**: Shell-specific implementations and hook generation
- **os**: Operating system-specific functionality
- **detect**: Platform and shell detection

### User Interface (`src/ui/`)

- **tui**: Terminal user interface components
- **tracing**: Structured logging and tracing
- **output**: Output filtering and formatting

### Utilities & Support (`src/utils/`)

- **sync**: Environment synchronization utilities
- **cleanup**: Resource cleanup handlers
- **network**: Network utilities and rate limiting
- **resilience**: Circuit breakers and retry logic
- **memory**: Memory management utilities
- **xdg**: XDG base directory specification compliance
- **compression**: Compression utilities
- **limits**: Resource limit management

## Implementation Phases

### Phase 1: Foundation (Low Risk, High Impact)

1. **Create Core Domain Structure**
   - Move `src/types.rs`, `src/errors.rs`, `src/constants.rs` to `src/core/`
   - Create `src/core/mod.rs` with proper re-exports
   - Update `src/lib.rs` to use new core structure

2. **Establish Utilities Module**
   - Create `src/utils/` structure
   - Move standalone utility modules (`sync_env.rs`, `cleanup.rs`, `xdg.rs`, etc.)
   - Ensure no circular dependencies

3. **Update Main Library Structure**
   - Simplify `src/lib.rs` to only re-export organized modules
   - Update all import statements throughout the codebase
   - Verify all tests still compile and pass

### Phase 2: Execution & Security (Medium Risk, Medium Impact)

1. **Organize Execution Engine**
   - Create `src/execution/` structure
   - Move task and command execution modules
   - Extract common patterns into reusable components

2. **Consolidate Security Modules**
   - Create `src/security/` structure
   - Group security-related functionality
   - Ensure security validation paths are clear

3. **Platform Integration**
   - Restructure `src/platform/` for better organization
   - Move shell-related modules into subdirectory
   - Improve platform abstraction layer

### Phase 3: Storage & UI (Medium Risk, Lower Impact)

1. **Storage Organization**
   - Reorganize `src/cache/` into `src/storage/cache/`
   - Move remote cache functionality
   - Consolidate file operations

2. **User Interface Structure**
   - Create `src/ui/` hierarchy
   - Organize TUI components logically
   - Group tracing and output functionality

### Phase 4: Finalization & Polish (Low Risk, Low Impact)

1. **Documentation Updates**
   - Update module documentation
   - Create architectural diagrams
   - Document migration guide

2. **Performance Optimization**
   - Analyze import patterns
   - Optimize compilation units
   - Reduce unnecessary dependencies

3. **Testing & Validation**
   - Ensure all tests pass
   - Run integration tests
   - Validate performance characteristics

## Key Benefits of This Refactoring

### 1. Improved Code Organization

- **Clear separation of concerns**: Each module has a well-defined responsibility
- **Logical grouping**: Related functionality is co-located
- **Reduced cognitive load**: Easier to navigate and understand

### 2. Better Maintainability

- **Targeted modifications**: Changes affect specific modules
- **Clear dependency chains**: Easier to understand impact of changes
- **Isolated testing**: Modules can be tested independently

### 3. Enhanced Reusability

- **Modular components**: Easier to reuse functionality
- **Clear interfaces**: Well-defined module boundaries
- **Pluggable architecture**: Easier to extend or replace components

### 4. Idiomatic Rust Patterns

- **Proper module structure**: Follows Rust conventions
- **Clear visibility rules**: Proper use of `pub` and privacy
- **Error handling consistency**: Centralized error types

## Risk Mitigation Strategies

### 1. Incremental Changes

- **Phase-based approach**: Implement in manageable chunks
- **Backward compatibility**: Maintain existing APIs during transition
- **Continuous testing**: Verify functionality at each step

### 2. Dependency Management

- **Clear layering**: Establish dependency hierarchy
- **Circular dependency prevention**: Tools to detect issues
- **Interface stability**: Define clear module contracts

### 3. Performance Considerations

- **Compilation impact**: Monitor compile times
- **Runtime performance**: Ensure no regressions
- **Memory usage**: Verify efficient resource utilization

## Success Metrics

### 1. Code Quality Metrics

- **Module cohesion**: High cohesion within modules
- **Coupling metrics**: Low coupling between modules
- **Complexity scores**: Reduced cyclomatic complexity

### 2. Developer Experience

- **Navigation time**: Reduced time to find functionality
- **Onboarding efficiency**: Faster understanding for new developers
- **Change confidence**: Higher confidence in making changes

### 3. Maintenance Metrics

- **Bug fix time**: Reduced time to address issues
- **Feature implementation time**: Faster feature development
- **Test coverage**: Maintained or improved test coverage

This refactoring plan establishes a solid foundation for the cuenv project's future growth and maintainability while following Rust best practices and idioms.
