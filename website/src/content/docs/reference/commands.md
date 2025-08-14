---
title: Command Reference
description: Complete reference for all cuenv commands
---

## Overview

cuenv provides a set of commands for managing environment configurations. All commands follow the pattern:

```bash
cuenv [command] [options] [arguments]
```

## Global Options

Options that work with all commands:

- `--help`, `-h` - Show help information
- `--version`, `-V` - Show version information
- `--cache <mode>` - Cache mode (off, read, read-write, write)
- `--cache-enabled <bool>` - Enable or disable caching globally
- `-e`, `--env <environment>` - Environment to use (e.g., dev, staging, production)
- `-c`, `--capability <capability>` - Capabilities to enable (can be specified multiple times)
- `--audit` - Run in audit mode to see file and network access without restrictions
- `--output-format <format>` - Output format for task execution (tui, spinner, simple, tree)
- `--trace-output <bool>` - Enable Chrome trace output

## Commands

### `cuenv`

Show help when no command is provided.

```bash
cuenv
```

### `cuenv init`

Initialize a new env.cue file with example configuration.

```bash
cuenv init [options]
```

**Options:**

- `-f`, `--force` - Force overwrite existing file

**Examples:**

```bash
# Create env.cue in current directory
cuenv init

# Overwrite existing env.cue
cuenv init --force
```

### `cuenv task`

List or execute tasks defined in your CUE configuration.

```bash
cuenv task [task_or_group] [args...]
```

**Options:**

- `-e`, `--env <environment>` - Use specific environment
- `-c`, `--capability <capability>` - Enable capabilities (can be specified multiple times)
- `--audit` - Run in audit mode to see file and network access
- `-v`, `--verbose` - Show detailed descriptions when listing
- `--output <format>` - Output format for task execution (tui, simple, spinner)
- `--trace-output` - Generate Chrome trace output file

**Examples:**

```bash
# List all tasks
cuenv task

# List tasks in a group
cuenv task build

# Execute a task
cuenv task build

# Execute a task with arguments
cuenv task test -- --coverage

# Execute with specific environment
cuenv task deploy -e production

# Execute with capabilities
cuenv task build -c aws -c docker
```

### `cuenv env`

Manage environment configuration and state.

#### `cuenv env allow`

Allow cuenv to load environments in a directory.

```bash
cuenv env allow [directory]
```

**Arguments:**

- `[directory]` - Directory to allow (default: current directory)

#### `cuenv env deny`

Deny cuenv from loading environments in a directory.

```bash
cuenv env deny [directory]
```

**Arguments:**

- `[directory]` - Directory to deny (default: current directory)

#### `cuenv env status`

Display current environment status and changes.

```bash
cuenv env status [options]
```

**Options:**

- `--hooks` - Show hooks status
- `-f`, `--format <format>` - Output format (human, starship, json)
- `-v`, `--verbose` - Show verbose output (for starship format)

#### `cuenv env export`

Export environment variables for the current directory.

```bash
cuenv env export [options]
```

**Options:**

- `-s`, `--shell <shell>` - Shell format (defaults to current shell)
- `--all` - Export all system environment variables, not just loaded ones

#### `cuenv env prune`

Prune stale environment state.

```bash
cuenv env prune
```

### `cuenv shell`

Configure shell integration for automatic environment loading.

#### `cuenv shell init`

Generate shell hook for automatic environment loading.

```bash
cuenv shell init <shell>
```

**Arguments:**

- `<shell>` - Shell type: `bash`, `zsh`, `fish`, etc.

**Examples:**

```bash
# Bash
eval "$(cuenv shell init bash)"

# Zsh
eval "$(cuenv shell init zsh)"

# Fish
cuenv shell init fish | source
```

#### `cuenv shell load`

Manually load environment from current directory.

```bash
cuenv shell load [options]
```

**Options:**

- `-d`, `--directory <directory>` - Directory to load from
- `-e`, `--env <environment>` - Environment to use
- `-c`, `--capability <capability>` - Capabilities to enable

#### `cuenv shell unload`

Manually unload current environment.

```bash
cuenv shell unload
```

#### `cuenv shell hook`

Generate shell hook for current directory.

```bash
cuenv shell hook [shell]
```

**Arguments:**

- `[shell]` - Shell name (defaults to current shell)

### `cuenv discover`

Discover all CUE packages in the repository.

```bash
cuenv discover [options]
```

**Options:**

- `--max-depth <depth>` - Maximum depth to search for env.cue files (default: 32)
- `-l`, `--load` - Load and validate discovered packages
- `-d`, `--dump` - Dump the CUE values for each package

### `cuenv cache`

Manage the task and environment cache.

#### `cuenv cache clear`

Clear all cache entries.

```bash
cuenv cache clear
```

#### `cuenv cache stats`

Show cache statistics.

```bash
cuenv cache stats
```

#### `cuenv cache cleanup`

Clean up stale cache entries.

```bash
cuenv cache cleanup [options]
```

**Options:**

- `--max-age-hours <hours>` - Maximum age of cache entries to keep (default: 168)

### `cuenv exec`

Execute a command with the loaded environment.

```bash
cuenv exec [options] <command> [args...]
```

**Options:**

- `-e`, `--env <environment>` - Environment to use
- `-c`, `--capability <capability>` - Capabilities to enable
- `--audit` - Run in audit mode

**Examples:**

```bash
# Run command with loaded environment
cuenv exec node server.js

# Run with specific environment
cuenv exec -e production npm start

# Run with capabilities
cuenv exec -c aws terraform apply
```

### `cuenv completion`

Generate shell completion scripts.

```bash
cuenv completion <shell>
```

**Arguments:**

- `<shell>` - Shell type: `bash`, `zsh`, `fish`, etc.

**Installation:**

```bash
# Bash
cuenv completion bash > /etc/bash_completion.d/cuenv

# Zsh
cuenv completion zsh > "${fpath[1]}/_cuenv"

# Fish
cuenv completion fish > ~/.config/fish/completions/cuenv.fish
```

### `cuenv mcp`

Start MCP (Model Context Protocol) server for Claude Code integration.

```bash
cuenv mcp [options]
```

**Options:**

- `--transport <transport>` - Transport type (stdio, tcp, unix) (default: stdio)
- `--port <port>` - TCP port (only for tcp transport) (default: 8765)
- `--socket <path>` - Unix socket path (only for unix transport)
- `--allow-exec` - Allow task execution (default: read-only)

**Examples:**

```bash
# Start MCP server with stdio transport (for Claude Code)
cuenv mcp

# Start with task execution enabled
cuenv mcp --allow-exec

# Start with Unix socket
cuenv mcp --transport unix --socket /tmp/cuenv.sock
```

## Exit Codes

cuenv uses standard exit codes:

- `0` - Success
- `1` - General error (command failed, file not found, etc.)
- Task exit codes are passed through from the executed command

## Environment Variables

### Variables Set by cuenv

- `CUENV_LOADED` - Set when an environment is loaded
- `CUENV_ROOT` - Path to the loaded env.cue file
- `CUENV_PREV_*` - Previous values of modified variables

### Configuration Variables

- `CUENV_FILE` - Custom filename (default: `env.cue`)
- `CUENV_DEBUG` - Enable debug output
- `CUENV_DISABLE_AUTO` - Disable automatic loading
- `CUENV_ENV` - Default environment for `cuenv exec`
- `CUENV_CAPABILITIES` - Default capabilities for `cuenv exec`

## Examples

### Basic Workflow

```bash
# Create environment file
cat > env.cue << 'EOF'
package env
DATABASE_URL: "postgres://localhost/myapp"
API_KEY: "dev-key-123"
EOF

# Load environment
cuenv load

# Check status
cuenv status

# Use variables
echo $DATABASE_URL

# Unload when done
cuenv unload
```

### Development vs Production

```bash
# env.cue with environments
cat > env.cue << 'EOF'
package env

PORT: 3000

environment: {
    production: {
        PORT: 8080
        DEBUG: false
    }
}
EOF

# Run development server
cuenv run -- node server.js

# Run production server
cuenv run -e production -- node server.js
```

### Secret Management

```bash
# env.cue with secrets
cat > env.cue << 'EOF'
package env

import "github.com/rawkode/cuenv/cue"

DATABASE_URL: "postgres://user:pass@localhost/db"
API_KEY: cuenv.#OnePasswordRef & {ref: "op://Work/MyApp/api_key"}
EOF

# Run with resolved secrets
cuenv run -- node app.js

# Secrets are obfuscated in output
cuenv run -- sh -c 'echo "Key: $API_KEY"'
# Output: Key: ***********
```

### CI/CD Usage

```bash
#!/bin/bash
# deploy.sh

# Load environment based on branch
if [[ "$GITHUB_REF" == "refs/heads/main" ]]; then
    ENV="production"
else
    ENV="staging"
fi

# Run deployment with appropriate environment
cuenv run -e "$ENV" -- terraform apply

# Run tests with CI environment
cuenv run -e ci -- npm test
```

### Docker Integration

```dockerfile
# Dockerfile
FROM node:16

# Install cuenv
RUN cargo install cuenv

# Copy application
COPY . /app
WORKDIR /app

# Run with cuenv
CMD ["cuenv", "run", "--", "node", "server.js"]
```

### Advanced Capability Usage

```bash
# Define capabilities in env.cue
cat > env.cue << 'EOF'
package env

# General variables
APP_NAME: "myapp"

# AWS credentials (only with 'aws' capability)
AWS_ACCESS_KEY: "key" @capability("aws")
AWS_SECRET_KEY: "secret" @capability("aws")

# Database (only with 'database' capability)
DATABASE_URL: "postgres://..." @capability("database")

# Capability mappings
capabilities: {
    aws: commands: ["terraform", "aws"]
    database: commands: ["terraform"]
}
EOF

# Automatic capability inference
cuenv run -- terraform plan  # Gets aws and database
cuenv run -- aws s3 ls       # Gets only aws

# Manual capability selection
cuenv run -c database -- ./migrate.sh
```

## Debugging

Enable debug output to troubleshoot issues:

```bash
# Enable debug mode
export CUENV_DEBUG=1

# Run command with debug output
cuenv load

# Or use debug flag
cuenv --debug status
```

Debug output includes:

- File paths being checked
- Environment inheritance chain
- Variable resolution details
- Secret manager calls
- Capability filtering decisions
