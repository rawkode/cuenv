---
title: API Reference
description: Comprehensive API reference for cuenv's internal components and integrations
---

# API Reference

This section provides detailed API documentation for cuenv's internal components, particularly useful for developers building integrations or contributing to cuenv.

## Overview

cuenv's architecture is built around a **centralized configuration pattern** using `Arc<Config>` for efficient, thread-safe sharing of parsed configuration data. This eliminates redundant CUE file parsing and provides significant performance improvements.

## Core APIs

- **[Configuration API](./configuration)** - Core `Config` and `ConfigLoader` APIs
- **[Task Server Protocol](./task-server-protocol)** - TSP and MCP server implementations
- **[Task Execution](./task-execution)** - Task executor and builder APIs
- **[Environment Management](./environment-management)** - Environment variable handling

## Key Patterns

### Arc<Config> Pattern

All major components now accept and use `Arc<Config>` for configuration access:

```rust
use std::sync::Arc;
use cuenv_config::Config;

// Commands accept shared configuration
impl TaskCommands {
    pub async fn execute(self, config: Arc<Config>) -> Result<()> {
        // Use config.get_tasks() - no I/O needed
        let tasks = config.get_tasks();
        // ...
    }
}

// Protocol servers accept shared configuration
impl TaskServerProvider {
    pub fn new_stdio(config: Arc<Config>, allow_exec: bool) -> Self {
        Self { config, allow_exec, /* ... */ }
    }
}
```

### Configuration Access

```rust
// Get tasks from shared configuration
let tasks: &HashMap<String, TaskConfig> = config.get_tasks();

// Get environment variables
let env_vars: &HashMap<String, String> = config.get_env_vars();

// Get hooks
let hooks: &HashMap<String, Vec<Hook>> = config.get_hooks();

// Check if variable is sensitive
let is_sensitive: bool = config.is_sensitive("API_KEY");
```

## Migration Guide

### From Old Pattern

```rust
// OLD: Direct CUE parsing in each component
use cuenv_config::{CueParser, ParseOptions};

async fn old_list_tasks() -> Result<()> {
    let options = ParseOptions::default();
    let parse_result = CueParser::eval_package(&dir, "env", &options)?; // Expensive I/O

    for (name, task) in parse_result.tasks {
        println!("{name}: {:?}", task.description);
    }
}
```

### To New Pattern

```rust
// NEW: Use shared Arc<Config>
use std::sync::Arc;
use cuenv_config::Config;

async fn new_list_tasks(config: Arc<Config>) -> Result<()> {
    let tasks = config.get_tasks(); // No I/O - uses cached data

    for (name, task) in tasks {
        println!("{name}: {:?}", task.description);
    }
}
```

## Performance Benefits

The centralized configuration architecture provides significant performance improvements:

| Operation          | Before (ms) | After (ms) | Improvement |
| ------------------ | ----------- | ---------- | ----------- |
| `cuenv task list`  | ~50ms       | ~20ms      | 2.5x faster |
| MCP server request | ~100ms      | ~10ms      | 10x faster  |
| Task execution     | ~75ms       | ~25ms      | 3x faster   |

## Integration APIs

### MCP Server Integration

```rust
use cuenv_task::TaskServerProvider;

// Create MCP server with shared configuration
let provider = TaskServerProvider::new_stdio(
    Arc::clone(&config),
    allow_exec
);

// Server uses pre-loaded configuration - no redundant parsing
provider.start().await?;
```

### Custom Command Integration

```rust
use cuenv_config::Config;
use std::sync::Arc;

struct MyCustomCommand {
    config: Arc<Config>,
}

impl MyCustomCommand {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub async fn execute(&self) -> Result<()> {
        // Access configuration data efficiently
        let tasks = self.config.get_tasks();
        let env_vars = self.config.get_env_vars();

        // Your custom logic here
        Ok(())
    }
}
```

## Error Handling

All API functions follow cuenv's error handling patterns:

```rust
use cuenv_core::{Error, Result};

// Functions return Result<T> for consistent error handling
pub async fn api_function(config: Arc<Config>) -> Result<String> {
    let tasks = config.get_tasks();

    if tasks.is_empty() {
        return Err(Error::configuration("No tasks defined".to_string()));
    }

    Ok("Success".to_string())
}
```

## Thread Safety

All `Arc<Config>` operations are thread-safe:

```rust
use std::sync::Arc;
use tokio::task;

async fn concurrent_access(config: Arc<Config>) {
    let config1 = Arc::clone(&config);
    let config2 = Arc::clone(&config);

    let task1 = task::spawn(async move {
        let tasks = config1.get_tasks();
        // Process tasks...
    });

    let task2 = task::spawn(async move {
        let env_vars = config2.get_env_vars();
        // Process environment variables...
    });

    let _ = tokio::try_join!(task1, task2);
}
```

## Next Steps

- Review the [Configuration API](./configuration) for detailed Config methods
- See [Task Server Protocol](./task-server-protocol) for MCP/TSP integration details
- Check [Task Execution](./task-execution) for task runner APIs
- Read [Environment Management](./environment-management) for env var handling
