---
title: MCP (Model Context Protocol) Server
description: Use cuenv with Claude Code and other MCP clients through the built-in MCP server
---

The MCP (Model Context Protocol) server allows Claude Code and other MCP clients to programmatically access cuenv environment variables and tasks. This integration provides a seamless way to manage environments and execute tasks directly from Claude Code.

## Overview

cuenv's MCP server extends the existing Task Server Protocol (TSP) infrastructure to support MCP methods alongside devenv compatibility. This unified approach provides:

- **Environment introspection**: List and query environment variables from `env.cue` configurations
- **Task discovery and execution**: Find and run tasks with proper security controls
- **Directory validation**: Check if directories contain valid cuenv configurations
- **Multi-transport support**: stdio, Unix socket, and TCP transport options

## Quick Start

### Basic Usage (Claude Code)

Start the MCP server in stdio mode (default for Claude Code integration):

```bash
cuenv mcp
```

This starts the server in read-only mode. For task execution, use:

```bash
cuenv mcp --allow-exec
```

### Transport Options

The MCP server supports multiple transport mechanisms:

```bash
# Stdio (default - for Claude Code)
cuenv mcp --transport stdio

# Unix socket
cuenv mcp --transport unix --socket /tmp/cuenv-mcp.sock

# TCP (creates internal Unix socket)
cuenv mcp --transport tcp --port 8765
```

### MCP Client Configuration

For Claude Code, add to your `.mcp.json`:

```json
{
  "servers": {
    "cuenv": {
      "command": "cuenv",
      "args": ["mcp", "--allow-exec"],
      "type": "stdio",
      "description": "cuenv environment and task management"
    }
  }
}
```

## Available Tools

The MCP server exposes the following tools to clients:

### Environment Tools

#### `cuenv.list_env_vars`
Lists all environment variables from the env.cue configuration.

**Parameters:**
- `directory` (required): Directory containing env.cue file
- `environment` (optional): Environment name (dev, staging, production, etc.)
- `capabilities` (optional): Array of capabilities to enable

**Example:**
```javascript
await useTool("cuenv.list_env_vars", {
  directory: "/home/user/myproject",
  environment: "dev"
});
```

#### `cuenv.get_env_var`
Gets the value of a specific environment variable.

**Parameters:**
- `directory` (required): Directory containing env.cue file  
- `name` (required): Environment variable name to retrieve
- `environment` (optional): Environment name
- `capabilities` (optional): Array of capabilities to enable

**Example:**
```javascript
await useTool("cuenv.get_env_var", {
  directory: "/home/user/myproject",
  name: "DATABASE_URL",
  environment: "dev"
});
```

### Task Tools

#### `cuenv.list_tasks`
Lists all available tasks from the env.cue configuration.

**Parameters:**
- `directory` (required): Directory containing env.cue file
- `environment` (optional): Environment name
- `capabilities` (optional): Array of capabilities to enable

**Example:**
```javascript
await useTool("cuenv.list_tasks", {
  directory: "/home/user/myproject"
});
```

#### `cuenv.get_task`
Gets details for a specific task.

**Parameters:**
- `directory` (required): Directory containing env.cue file
- `name` (required): Task name to retrieve  
- `environment` (optional): Environment name
- `capabilities` (optional): Array of capabilities to enable

**Example:**
```javascript
await useTool("cuenv.get_task", {
  directory: "/home/user/myproject",
  name: "build"
});
```

#### `cuenv.run_task` (requires --allow-exec)
Executes a task. Only available when the server is started with `--allow-exec`.

**Parameters:**
- `directory` (required): Directory containing env.cue file
- `name` (required): Task name to execute
- `args` (optional): Array of arguments to pass to the task
- `environment` (optional): Environment name
- `capabilities` (optional): Array of capabilities to enable

**Example:**
```javascript
await useTool("cuenv.run_task", {
  directory: "/home/user/myproject", 
  name: "build",
  args: ["--release"]
});
```

### Discovery Tools

#### `cuenv.check_directory`
Validates if a directory contains an env.cue file and is accessible.

**Parameters:**
- `directory` (required): Directory path to check

**Example:**
```javascript
await useTool("cuenv.check_directory", {
  directory: "/home/user/myproject"
});
```

## Security Model

The MCP server implements several security measures:

### Execution Control
- **Read-only by default**: Environment variables and task definitions can be queried without the `--allow-exec` flag
- **Execution gate**: Task execution requires explicit `--allow-exec` flag
- **Directory validation**: All operations require explicit directory parameters (no implicit context)

### Access Patterns
- **Explicit directories**: Every tool call must specify the target directory
- **No implicit state**: The server doesn't maintain directory context between calls
- **Environment isolation**: Each tool call uses read-only environment parsing to avoid side effects

## Advanced Configuration

### Multiple Environment Support

The MCP server supports querying different environments from the same project:

```javascript
// Development environment
await useTool("cuenv.list_env_vars", {
  directory: "/home/user/myproject",
  environment: "dev"
});

// Production environment  
await useTool("cuenv.list_env_vars", {
  directory: "/home/user/myproject",
  environment: "production"
});
```

### Capability-based Filtering

Use capabilities to enable specific features:

```javascript
await useTool("cuenv.list_env_vars", {
  directory: "/home/user/myproject",
  capabilities: ["network", "filesystem"]
});
```

## Protocol Compatibility

The MCP server maintains full backward compatibility with the existing Task Server Protocol (TSP) used by devenv. The same server instance can handle both:

- **MCP methods**: `tools/list`, `tools/call` (for Claude Code)
- **TSP methods**: `initialize`, `run` (for devenv)

This unified approach ensures that existing devenv integrations continue to work while adding MCP support.

## Troubleshooting

### Common Issues

**"Directory not found" errors:**
- Ensure the directory path is absolute
- Check that the directory exists and contains an env.cue file

**"Task execution not allowed" errors:**
- Start the server with `--allow-exec` flag to enable task execution
- Verify the task exists in the specified directory

**Transport errors:**
- For stdio transport, ensure no other process is using stdin/stdout
- For Unix socket transport, check that the socket path is accessible
- For TCP transport, note that it currently uses internal Unix sockets

### Debug Mode

For debugging MCP server issues, you can:

1. Check server logs for detailed error messages
2. Verify directory permissions with `cuenv.check_directory`
3. List available tasks with `cuenv.list_tasks` before execution

## Examples

### Complete Claude Code Integration

Here's a complete example of using cuenv with Claude Code:

1. **Setup .mcp.json:**
```json
{
  "servers": {
    "cuenv": {
      "command": "cuenv", 
      "args": ["mcp", "--allow-exec"],
      "type": "stdio",
      "description": "cuenv environment and task management"
    }
  }
}
```

2. **Query environment in Claude Code:**
```javascript
// Check if directory has cuenv configuration
const check = await useTool("cuenv.check_directory", {
  directory: "/home/user/myproject"
});

// List all environment variables
const envVars = await useTool("cuenv.list_env_vars", {
  directory: "/home/user/myproject",
  environment: "dev"
});

// List available tasks
const tasks = await useTool("cuenv.list_tasks", {
  directory: "/home/user/myproject"
});

// Execute a build task
const result = await useTool("cuenv.run_task", {
  directory: "/home/user/myproject",
  name: "build",
  args: ["--release"]
});
```

This integration allows Claude Code to fully manage your cuenv environments and execute tasks programmatically, providing a powerful development workflow automation capability.