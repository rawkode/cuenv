---
title: MCP (Model Context Protocol) Integration
description: Using cuenv with MCP clients like Claude Code for programmatic environment and task management
---

## Overview

cuenv provides a Model Context Protocol (MCP) server that exposes environment management and task execution capabilities to MCP clients like Claude Code. This enables AI assistants to programmatically access your environment variables and execute tasks while maintaining security boundaries.

## Starting the MCP Server

### Basic Usage

Start the MCP server with stdio transport (default):

```bash
cuenv mcp
```

### TCP Transport

Start the server with TCP transport on a specific port:

```bash
cuenv mcp --transport tcp --port 8080
```

### Enable Task Execution

By default, task execution is disabled for security. To allow task execution:

```bash
cuenv mcp --allow-exec
```

## Available Tools

The MCP server provides 8 tools for interacting with cuenv:

### Environment Tools

#### `list_env_vars`

Lists all environment variables for a directory.

**Parameters:**

- `directory` (required): Directory containing env.cue file
- `environment` (optional): Environment name (dev, staging, production, etc.)
- `capabilities` (optional): List of capabilities to enable

#### `get_env_var`

Gets a specific environment variable value.

**Parameters:**

- `directory` (required): Directory containing env.cue file
- `name` (required): Environment variable name to retrieve
- `environment` (optional): Environment name
- `capabilities` (optional): List of capabilities to enable

#### `list_environments`

Lists available environments (extracted from CUE schema).

**Parameters:**

- `directory` (required): Directory containing env.cue file

### Task Tools

#### `list_tasks`

Lists all available tasks.

**Parameters:**

- `directory` (required): Directory containing env.cue file
- `environment` (optional): Environment name
- `capabilities` (optional): List of capabilities to enable

#### `get_task`

Gets details for a specific task.

**Parameters:**

- `directory` (required): Directory containing env.cue file
- `name` (required): Task name to retrieve
- `environment` (optional): Environment name
- `capabilities` (optional): List of capabilities to enable

#### `run_task`

Executes a task (requires `--allow-exec` flag).

**Parameters:**

- `directory` (required): Directory containing env.cue file
- `name` (required): Task name to execute
- `args` (optional): Arguments to pass to the task
- `environment` (optional): Environment name
- `capabilities` (optional): List of capabilities to enable

### Discovery Tools

#### `check_directory`

Checks if a directory is valid and allowed.

**Parameters:**

- `directory` (required): Directory path to check

#### `list_capabilities`

Lists available capabilities (extracted from CUE metadata).

**Parameters:**

- `directory` (required): Directory containing env.cue file

## Security Model

### Directory Validation

All MCP tools require explicit directory parameters for security. Each tool call validates:

1. **Directory exists** and is accessible
2. **Directory is allowed** via cuenv's permission system
3. **Path canonicalization** to prevent path traversal attacks

### Permission System

The MCP server integrates with cuenv's existing directory permission system:

```bash
# Allow a directory for MCP access
cuenv allow /path/to/project

# List allowed directories
cuenv allowed
```

### Task Execution Guard

Task execution requires the `--allow-exec` flag when starting the MCP server. This prevents accidental code execution through MCP clients.

### Read-Only Environment Parsing

Environment parsing for MCP tools is read-only and doesn't affect the server's environment or create side effects.

## Usage with Claude Code

### Configuration

Add cuenv MCP server to your Claude Code configuration:

```json
{
	"mcpServers": {
		"cuenv": {
			"command": "cuenv",
			"args": ["mcp", "--allow-exec"]
		}
	}
}
```

### Example Interactions

With the MCP server running, you can ask Claude Code to:

- "List all environment variables in this project"
- "What tasks are available in this directory?"
- "Run the build task"
- "Show me the staging environment configuration"
- "What capabilities are defined in this project?"

## Transport Options

### stdio (Default)

Best for integration with MCP clients that spawn processes:

```bash
cuenv mcp
```

### TCP

Useful for persistent server instances or remote access:

```bash
cuenv mcp --transport tcp --port 8080
```

## Error Handling

The MCP server provides detailed error messages for common scenarios:

- **Directory not found**: Clear message with path information
- **Directory not allowed**: Instructions on how to allow the directory
- **Task execution disabled**: Guidance on using `--allow-exec` flag
- **Invalid CUE files**: Parsing errors with context

## Best Practices

1. **Use directory allowlist**: Always use `cuenv allow` to explicitly permit MCP access to directories
2. **Selective task execution**: Only use `--allow-exec` when you need task execution capabilities
3. **Environment isolation**: Use specific environments and capabilities to limit access scope
4. **Regular permission review**: Periodically review allowed directories with `cuenv allowed`

## Troubleshooting

### "Directory not allowed" errors

```bash
# Check current allowed directories
cuenv allowed

# Allow the directory
cuenv allow /path/to/project
```

### "Task execution not allowed" errors

Restart the MCP server with task execution enabled:

```bash
cuenv mcp --allow-exec
```

### Connection issues with TCP transport

- Verify the port is not in use by another process
- Check firewall settings if accessing remotely
- Ensure the client is connecting to the correct port

## Integration Examples

### Basic Environment Query

```javascript
// MCP client code example
const envVars = await callTool("list_env_vars", {
	directory: "/path/to/project",
	environment: "dev",
});
```

### Task Execution

```javascript
// Execute a build task
const result = await callTool("run_task", {
	directory: "/path/to/project",
	name: "build",
	environment: "dev",
});
```

The MCP integration makes cuenv's powerful environment and task management capabilities available to AI assistants while maintaining security and explicit control over access permissions.
