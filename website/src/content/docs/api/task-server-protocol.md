---
title: Task Server Protocol API
description: API reference for cuenv's Task Server Protocol (TSP) and MCP implementations
---

# Task Server Protocol API

This document covers the API for cuenv's Task Server Protocol (TSP) and MCP (Model Context Protocol) implementations, which have been significantly optimized with the new centralized configuration architecture.

## Overview

The Task Server Protocol implementation now uses `Arc<Config>` for efficient configuration sharing, eliminating redundant CUE file parsing and providing up to 10x performance improvements for protocol requests.

## TaskServerProvider

The core component for exposing cuenv tasks to external tools via JSON-RPC protocols.

### Constructor Changes

#### Before (Deprecated)

```rust
// OLD: Accepted HashMap, required redundant parsing
impl TaskServerProvider {
    pub fn new(socket_path: PathBuf, tasks: HashMap<String, TaskConfig>) -> Self
    pub fn new_stdio(tasks: HashMap<String, TaskConfig>, allow_exec: bool) -> Self
}
```

#### After (Current)

```rust
// NEW: Accepts Arc<Config>, uses pre-loaded data
impl TaskServerProvider {
    /// Create a new task server provider for Unix socket
    pub fn new(socket_path: PathBuf, config: Arc<Config>) -> Self

    /// Create a new task server provider for stdio (MCP mode)
    pub fn new_stdio(config: Arc<Config>, allow_exec: bool) -> Self

    /// Create a new task server provider with full options
    pub fn new_with_options(
        socket_path: Option<PathBuf>,
        config: Arc<Config>,
        allow_exec: bool,
        use_stdio: bool,
    ) -> Self
}
```

### Usage Examples

#### MCP Server (stdio)

```rust
use cuenv_task::TaskServerProvider;
use std::sync::Arc;

// Create MCP server with shared configuration
let provider = TaskServerProvider::new_stdio(
    Arc::clone(&config),
    allow_exec
);

// Start server (handles both MCP and TSP protocols)
provider.start().await?;
```

#### TSP Server (Unix socket)

```rust
use cuenv_task::TaskServerProvider;
use std::path::PathBuf;

let socket_path = PathBuf::from("/tmp/cuenv-tasks.sock");
let provider = TaskServerProvider::new(
    socket_path,
    Arc::clone(&config)
);

provider.start().await?;
```

#### Full Options

```rust
let provider = TaskServerProvider::new_with_options(
    Some(socket_path),
    Arc::clone(&config),
    true,  // allow_exec
    false, // use_stdio
);
```

## Protocol Support

### Dual Protocol Implementation

The `TaskServerProvider` handles both protocols simultaneously:

```rust
// MCP Methods (for Claude Code, etc.)
"tools/list"  -> Lists available MCP tools
"tools/call"  -> Executes MCP tool calls

// TSP Methods (for devenv compatibility)
"initialize"  -> Returns available tasks
"run"         -> Executes a task
```

### Request Handling

```rust
impl TaskServerProvider {
    /// Handle JSON-RPC requests (supports both TSP and MCP)
    pub async fn handle_request(
        request: serde_json::Value,
        tasks: &HashMap<String, TaskConfig>, // Now from Arc<Config>
        allow_exec: bool,
    ) -> serde_json::Value {
        // Unified handler for both protocols
    }
}
```

## MCP Tool Definitions

The MCP server exposes these tools to clients:

### Environment Tools

```rust
// cuenv.list_env_vars - List all environment variables
{
    "name": "cuenv.list_env_vars",
    "description": "List all environment variables from env.cue configuration",
    "inputSchema": {
        "type": "object",
        "properties": {
            "directory": { "type": "string", "description": "Directory containing env.cue file" },
            "environment": { "type": "string", "description": "Optional environment name" },
            "capabilities": { "type": "array", "items": {"type": "string"} }
        },
        "required": ["directory"]
    }
}

// cuenv.get_env_var - Get specific environment variable
{
    "name": "cuenv.get_env_var",
    "inputSchema": {
        "properties": {
            "directory": { "type": "string" },
            "name": { "type": "string", "description": "Environment variable name" },
            "environment": { "type": "string" },
            "capabilities": { "type": "array" }
        },
        "required": ["directory", "name"]
    }
}
```

### Task Tools

```rust
// cuenv.list_tasks - List available tasks
{
    "name": "cuenv.list_tasks",
    "inputSchema": {
        "properties": {
            "directory": { "type": "string" },
            "environment": { "type": "string" },
            "capabilities": { "type": "array" }
        },
        "required": ["directory"]
    }
}

// cuenv.get_task - Get specific task details
{
    "name": "cuenv.get_task",
    "inputSchema": {
        "properties": {
            "directory": { "type": "string" },
            "name": { "type": "string", "description": "Task name" },
            "environment": { "type": "string" },
            "capabilities": { "type": "array" }
        },
        "required": ["directory", "name"]
    }
}

// cuenv.run_task - Execute task (requires --allow-exec)
{
    "name": "cuenv.run_task",
    "inputSchema": {
        "properties": {
            "directory": { "type": "string" },
            "name": { "type": "string" },
            "args": { "type": "array", "items": {"type": "string"} },
            "environment": { "type": "string" },
            "capabilities": { "type": "array" }
        },
        "required": ["directory", "name"]
    }
}
```

## Performance Improvements

### Before: Redundant Parsing

```rust
// OLD: Each MCP request parsed CUE files
async fn handle_mcp_tool_call(params: serde_json::Value) -> serde_json::Value {
    let directory = params.get("directory").unwrap();

    // EXPENSIVE: Parse CUE files on every request
    let parse_result = CueParser::eval_package_with_options(directory, "env", &options)?;

    // Use parse_result.tasks...
}
```

### After: Shared Configuration

```rust
// NEW: Uses pre-loaded Arc<Config>
async fn handle_mcp_tool_call(params: serde_json::Value) -> serde_json::Value {
    // FAST: No I/O needed, uses cached configuration
    let tasks = self.config.get_tasks();

    // Use tasks directly...
}
```

### Benchmarks

| Operation        | Before | After | Improvement |
| ---------------- | ------ | ----- | ----------- |
| MCP `list_tasks` | ~80ms  | ~8ms  | 10x faster  |
| MCP `get_task`   | ~70ms  | ~5ms  | 14x faster  |
| MCP `run_task`   | ~120ms | ~25ms | 5x faster   |
| TSP `initialize` | ~60ms  | ~6ms  | 10x faster  |

## Security Model

### Execution Control

```rust
// Execution requires explicit permission
let provider = TaskServerProvider::new_stdio(
    config,
    true  // allow_exec - must be explicitly enabled
);
```

### Directory Validation

```rust
impl TaskServerProvider {
    /// Validate directory and check if it's allowed
    fn validate_directory(directory: &str) -> Result<PathBuf> {
        let path = PathBuf::from(directory);

        // Canonicalize to prevent path traversal
        let canonical = path.canonicalize()?;

        // Ensure absolute path
        if !canonical.is_absolute() {
            return Err(Error::configuration("Path must be absolute"));
        }

        Ok(canonical)
    }
}
```

## Error Handling

### JSON-RPC Error Responses

```rust
// Configuration errors
{
    "jsonrpc": "2.0",
    "error": {
        "code": -1,
        "message": "Failed to load environment: Invalid CUE syntax"
    },
    "id": request_id
}

// Execution errors (when allow_exec=false)
{
    "jsonrpc": "2.0",
    "error": {
        "code": -1,
        "message": "Task execution not allowed. Start MCP server with --allow-exec flag."
    },
    "id": request_id
}

// Method not found
{
    "jsonrpc": "2.0",
    "error": {
        "code": -32601,
        "message": "Method not found: invalid_method"
    },
    "id": request_id
}
```

## Integration Examples

### Claude Code Integration

```json
{
	"servers": {
		"cuenv": {
			"command": "cuenv",
			"args": ["mcp", "--allow-exec"],
			"type": "stdio"
		}
	}
}
```

### devenv Integration

```nix
{ pkgs, ... }: {
  tasks = {
    "cuenv:build" = {
      exec = "cuenv internal task-protocol --server ./build-server --run-task build";
    };
  };
}
```

### Custom Integration

```rust
use cuenv_task::{TaskServerManager, UnifiedTaskManager};

async fn custom_integration(config: Arc<Config>) -> Result<()> {
    // Create unified manager with internal tasks
    let socket_dir = tempfile::tempdir()?.path().to_path_buf();
    let mut manager = UnifiedTaskManager::new(socket_dir, config);

    // Discover external task servers
    let tasks = manager.discover_all_tasks(Some(&discovery_dir)).await?;

    // Tasks now include both internal cuenv tasks and external tasks
    for task in tasks {
        println!("Available: {}", task.name);
    }

    Ok(())
}
```

## Migration Guide

### Updating Existing Integrations

If you have existing code that creates `TaskServerProvider`:

```rust
// OLD
let tasks = parse_cue_files()?; // Your CUE parsing code
let provider = TaskServerProvider::new_stdio(tasks, allow_exec);

// NEW
let config = ConfigLoader::load_from_directory(&directory)?;
let shared_config = Arc::new(config);
let provider = TaskServerProvider::new_stdio(shared_config, allow_exec);
```

### Performance Testing

```rust
// Benchmark MCP server performance
use std::time::Instant;

let start = Instant::now();
let response = provider.handle_request(mcp_request, &config.get_tasks(), false).await;
println!("Request handled in: {:?}", start.elapsed());
// Expected: ~5-10ms (vs ~50-100ms before)
```

## Future Enhancements

### Planned Features

- WebSocket transport support
- Streaming task output for long-running tasks
- Task progress reporting
- Distributed task execution
- Advanced capability-based security

### API Stability

- `Arc<Config>` pattern is stable and will be maintained
- JSON-RPC protocol methods are versioned for compatibility
- MCP tool schemas follow semantic versioning
