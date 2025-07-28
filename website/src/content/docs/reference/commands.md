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
- `--debug` - Enable debug output

## Commands

### `cuenv`

Check if an `env.cue` file exists in the current directory.

```bash
cuenv
```

**Output:**

- Success: "env.cue found in current directory"
- Failure: "No env.cue found in current directory"

### `cuenv init`

Generate shell initialization script for automatic environment loading.

```bash
cuenv init <shell>
```

**Arguments:**

- `<shell>` - Shell type: `bash`, `zsh`, or `fish`

**Examples:**

```bash
# Bash
eval "$(cuenv init bash)"

# Zsh
eval "$(cuenv init zsh)"

# Fish
cuenv init fish | source
```

### `cuenv allow`

Allow a directory to load environment files.

```bash
cuenv allow [directory]
```

**Arguments:**

- `[directory]` - Directory to allow (default: current directory)

**Examples:**

```bash
# Allow current directory
cuenv allow

# Allow specific directory
cuenv allow /path/to/project

# Allow parent directory
cuenv allow ..
```

**Notes:**

- Uses SHA256 hashing to track file content
- Required before cuenv will load `env.cue` files
- Changes to allowed files reload automatically
- Approval persists across sessions

### `cuenv load`

Manually load environment from a directory.

```bash
cuenv load [directory]
```

**Arguments:**

- `[directory]` - Directory to load from (default: current directory)

**Examples:**

```bash
# Load from current directory
cuenv load

# Load from specific directory
cuenv load /path/to/project

# Load from parent directory
cuenv load ..
```

### `cuenv unload`

Unload the currently loaded environment.

```bash
cuenv unload
```

**Notes:**

- Restores previous environment variables
- Safe to call even if no environment is loaded

### `cuenv status`

Show the current environment status and changes.

```bash
cuenv status
```

**Output includes:**

- Currently loaded environment path
- List of set variables
- List of modified variables
- List of unset variables

**Example output:**

```
Environment loaded from: /home/user/project
Set variables:
  DATABASE_URL=postgres://localhost/myapp
  PORT=3000
Modified variables:
  PATH=/home/user/project/bin:/usr/bin:/bin
Unset variables:
  OLD_VAR
```

### `cuenv hook`

Generate shell-specific hook output for directory change detection.

```bash
cuenv hook <shell>
```

**Arguments:**

- `<shell>` - Shell type: `bash`, `zsh`, or `fish`

**Notes:**

- Used internally by `cuenv init`
- Can be used for custom shell integration
- Outputs shell commands that should be evaluated

### `cuenv run`

Run a command in a hermetic environment with only CUE-defined variables.

```bash
cuenv run [options] -- <command> [args...]
```

**Options:**

- `-e`, `--env <environment>` - Use specific environment
- `-c`, `--capabilities <list>` - Enable capabilities (comma-separated)
- `--` - Separator between cuenv options and command

**Environment variables:**

- `CUENV_ENV` - Set default environment
- `CUENV_CAPABILITIES` - Set default capabilities

**Examples:**

```bash
# Run with base environment
cuenv run -- node server.js

# Run with production environment
cuenv run -e production -- npm start

# Run with specific capabilities
cuenv run -c aws,database -- terraform apply

# Pass arguments to command
cuenv run -- npm -- install --save-dev

# Use environment variables
CUENV_ENV=staging cuenv run -- ./deploy.sh
```

**Secret Resolution:**

- Resolves 1Password references (`op://...`)
- Resolves GCP Secret Manager references (`gcp-secret://...`)
- Automatically obfuscates secret values in output

### `cuenv export`

Export the current environment in various formats.

```bash
cuenv export [options]
```

**Options:**

- `-f`, `--format <format>` - Output format: `shell`, `json`, `dotenv`, `docker`
- `-e`, `--env <environment>` - Use specific environment

**Examples:**

```bash
# Export as shell script
cuenv export -f shell > env.sh

# Export as JSON
cuenv export -f json > env.json

# Export as .env file
cuenv export -f dotenv > .env

# Export for Docker
cuenv export -f docker > docker.env
```

### `cuenv dump`

Print the current environment state to stdout.

```bash
cuenv dump
```

**Notes:**

- Shows all currently loaded environment variables
- Used for debugging and state inspection
- Output format is suitable for shell evaluation

### `cuenv prune`

Clean up stale state files and caches.

```bash
cuenv prune [options]
```

**Options:**

- `--all` - Remove all state files, not just stale ones
- `--dry-run` - Show what would be removed without removing

**Examples:**

```bash
# Remove stale state files
cuenv prune

# See what would be removed
cuenv prune --dry-run

# Remove all state files
cuenv prune --all
```

### `cuenv completion`

Generate shell completion scripts.

```bash
cuenv completion <shell>
```

**Arguments:**

- `<shell>` - Shell type: `bash`, `zsh`, or `fish`

**Installation:**

```bash
# Bash
cuenv completion bash > /etc/bash_completion.d/cuenv

# Zsh
cuenv completion zsh > "${fpath[1]}/_cuenv"

# Fish
cuenv completion fish > ~/.config/fish/completions/cuenv.fish
```

### `cuenv remote-cache-server`

Start a remote cache server compatible with Bazel/Buck2 builds.

```bash
cuenv remote-cache-server [options]
```

**Options:**

- `-a`, `--address <address>` - Address to listen on (default: `127.0.0.1:50051`)
- `-c`, `--cache-dir <path>` - Cache storage directory (default: `/var/cache/cuenv`)
- `--max-cache-size <bytes>` - Maximum cache size in bytes (default: `10737418240`)

**Examples:**

```bash
# Start with default settings
cuenv remote-cache-server

# Start on public interface
cuenv remote-cache-server --address 0.0.0.0:50051

# Use custom cache directory
cuenv remote-cache-server --cache-dir /data/bazel-cache

# Set 50GB cache limit
cuenv remote-cache-server --max-cache-size 53687091200
```

**Notes:**

- Implements Bazel/Buck2 Remote Execution API protocol
- Provides Content-Addressed Storage (CAS) service
- Provides Action Cache service for build results
- Uses cuenv's lock-free concurrent cache infrastructure
- See the [Remote Cache Server guide](/guides/remote-cache-server) for deployment options

## Exit Codes

cuenv uses standard exit codes:

- `0` - Success
- `1` - General error
- `2` - Usage error (invalid arguments)
- `3` - Environment not found
- `4` - CUE validation error
- `5` - Secret resolution error

## Environment Variables

### Variables Set by cuenv

- `CUENV_LOADED` - Set when an environment is loaded
- `CUENV_ROOT` - Path to the loaded env.cue file
- `CUENV_PREV_*` - Previous values of modified variables

### Configuration Variables

- `CUENV_FILE` - Custom filename (default: `env.cue`)
- `CUENV_DEBUG` - Enable debug output
- `CUENV_DISABLE_AUTO` - Disable automatic loading
- `CUENV_ENV` - Default environment for `cuenv run`
- `CUENV_CAPABILITIES` - Default capabilities for `cuenv run`

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
