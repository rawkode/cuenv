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

# Command mappings
Commands: {
    terraform: capabilities: ["aws", "database"]
    aws: capabilities: ["aws"]
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
