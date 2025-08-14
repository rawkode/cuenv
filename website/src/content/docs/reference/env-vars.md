---
title: Environment Variables Reference
description: Complete list of environment variables used by cuenv
---

## Configuration Variables

These environment variables control cuenv's behavior.

### CUENV_ENV

Selects which environment configuration to load.

- **Type:** String
- **Default:** `default`
- **Examples:** `development`, `staging`, `production`

```bash
# Set environment
export CUENV_ENV="production"

# Or use with command
cuenv exec -e staging -- npm start
```

### CUENV_FORMAT

Sets the output format for various commands.

- **Type:** String
- **Default:** `shell`
- **Values:** `shell`, `json`, `export`, `dotenv`

```bash
# Output as JSON
cuenv show -f json

# Output as dotenv format
export CUENV_FORMAT="dotenv"
cuenv show
```

### CUENV_LOG_LEVEL

Controls the verbosity of logging output.

- **Type:** String
- **Default:** `error`
- **Values:** `trace`, `debug`, `info`, `warn`, `error`

```bash
# Enable debug logging
export CUENV_LOG_LEVEL="debug"

# See detailed loading information
cuenv load
```

### CUENV_CAPABILITIES

Sets security capabilities for CUE evaluation.

- **Type:** String
- **Default:** `safe`
- **Values:**
  - `none` - No capabilities (most restrictive)
  - `safe` - Safe built-in functions only
  - `read` - File reading allowed
  - `write` - File writing allowed
  - `net` - Network access allowed
  - `exec` - Command execution allowed
  - `all` - All capabilities (least restrictive)

```bash
# Allow file reading and network access
export CUENV_CAPABILITIES="read,net"

# Maximum security (no capabilities)
export CUENV_CAPABILITIES="none"
```

### CUENV_NO_HOOK

Disables shell hook functionality.

- **Type:** Boolean (presence check)
- **Default:** Not set (hooks enabled)

```bash
# Disable shell hooks temporarily
export CUENV_NO_HOOK=1

# Re-enable hooks
unset CUENV_NO_HOOK
```

### CUENV_FILE

Custom environment file name.

- **Type:** String
- **Default:** `env.cue`
- **Examples:** `environment.cue`, `.env.cue`, `config.cue`

```bash
# Use custom filename
export CUENV_FILE="environment.cue"

# Now cuenv looks for environment.cue instead of env.cue
cuenv load
```

### CUENV_DISABLE_AUTO

Disables automatic environment loading on directory change.

- **Type:** Boolean (presence check)
- **Default:** Not set (auto-loading enabled)

```bash
# Disable automatic loading
export CUENV_DISABLE_AUTO=1

# Manual loading required
cd /path/to/project
cuenv load  # Must be run manually
```

### CUENV_DEBUG

Enables debug output (alias for CUENV_LOG_LEVEL=debug).

- **Type:** Boolean (presence check)
- **Default:** Not set

```bash
# Enable debug mode
export CUENV_DEBUG=1

# Same as
export CUENV_LOG_LEVEL=debug
```

## Runtime Variables

These variables are set by cuenv during operation.

### CUENV_DIR

The directory containing the currently loaded environment.

- **Type:** String (path)
- **Set by:** `cuenv` when environment is loaded
- **Used for:** Tracking current environment directory

```bash
# Check current environment directory
echo "Environment directory: $CUENV_DIR"
```

### CUENV_FILE

The path to the currently loaded environment file.

- **Type:** String (file path)
- **Set by:** `cuenv` when environment is loaded
- **Used for:** Identifying exact file loaded

```bash
# Check loaded file
echo "Loaded from: $CUENV_FILE"
```

### CUENV_WATCHES

Colon-separated list of watched files for auto-reload.

- **Type:** String (colon-separated paths)
- **Set by:** `cuenv` based on file imports and dependencies
- **Used for:** Tracking files that trigger reload on change

```bash
# View watched files
echo "$CUENV_WATCHES" | tr ':' '\n'
```

### CUENV_DIFF

Base64-encoded environment diff for restoration.

- **Type:** String (base64)
- **Set by:** `cuenv` when modifying environment
- **Internal use:** For restoring previous environment state

### CUENV_ROOT

The directory containing the loaded environment file (legacy, same as CUENV_DIR).

- **Type:** String (path)
- **Set by:** `cuenv shell load` when a file is found
- **Used for:** Resolving relative paths, identifying project root

```bash
# Check which project is loaded
echo "Environment loaded from: $CUENV_ROOT"

# Use in scripts
if [[ "$CUENV_ROOT" == "$HOME/projects/myapp" ]]; then
    echo "In myapp project"
fi
```

### CUENV_PREV\_\*

Stores previous values of environment variables that were modified.

- **Type:** String
- **Format:** `CUENV_PREV_<VARIABLE_NAME>`
- **Set by:** `cuenv load` (for modified variables)
- **Used by:** `cuenv unload` (to restore values)

Example:

```bash
# Original environment
export DATABASE_URL="postgres://localhost/dev"

# Load env.cue that sets DATABASE_URL="postgres://localhost/prod"
cuenv shell load

# cuenv automatically sets:
# CUENV_PREV_DATABASE_URL="postgres://localhost/dev"

# Unload restores original value
cuenv shell unload
echo $DATABASE_URL  # "postgres://localhost/dev"
```

## Shell Integration Variables

Variables used by shell hooks and integrations.

### CUENV_SHELL

Identifies the current shell for proper hook installation.

- **Type:** String
- **Values:** `bash`, `zsh`, `fish`
- **Set by:** `cuenv init <shell>`

```bash
# Check current shell integration
echo $CUENV_SHELL

# Reinitialize for different shell
eval "$(cuenv init zsh)"
```

### \_CUENV_PWD

Tracks directory changes for automatic environment loading.

- **Type:** String (path)
- **Set by:** Shell hook
- **Internal use only**

## Task Execution Variables

Variables used for task execution and workflow management.

### CUENV_CAPABILITIES

Comma-separated list of capabilities enabled for task execution.

- **Type:** String (comma-separated)
- **Used by:** Task executor for capability-based access control

```bash
export CUENV_CAPABILITIES="network,filesystem"

# Tasks can check capabilities
cuenv task deploy  # Uses network and filesystem capabilities
```

### CUENV_CACHE_MODE

Controls caching behavior during task execution.

- **Type:** String
- **Values:** `off`, `read`, `read-write`, `write`
- **Default:** `read-write`

```bash
export CUENV_CACHE_MODE="read"

# Only read from cache, don't write to it
cuenv task build
```

### CUENV_CACHE_ENABLED

Enable or disable task result caching.

- **Type:** Boolean string
- **Values:** `true`, `false`
- **Default:** `true`

```bash
export CUENV_CACHE_ENABLED="false"

# Disable all caching
cuenv task test
```

## Command-Specific Variables

### CUENV_OUTPUT_FORMAT

Controls the output format for task execution.

- **Type:** String
- **Values:** `tui`, `spinner`, `simple`, `tree`
- **Default:** `spinner`

```bash
export CUENV_OUTPUT_FORMAT="tui"

# Use TUI interface for task execution
cuenv task build
```

## Best Practices

### Setting Defaults

```bash title="~/.bashrc"
# Set sensible defaults in shell config
export CUENV_LOG_LEVEL="${CUENV_LOG_LEVEL:-error}"
export CUENV_FORMAT="${CUENV_FORMAT:-shell}"
export CUENV_CAPABILITIES="${CUENV_CAPABILITIES:-safe}"
```

## Precedence Rules

When multiple sources set the same variable:

1. Command-line flags (highest priority)
1. Environment variables
1. Configuration file values
1. Default values (lowest priority)

Example:

```bash
# env.cue defines environment-specific values
# Shell has: export CUENV_ENV=staging

cuenv exec -- echo $PORT                    # Uses staging
cuenv exec -e production -- echo $PORT      # Uses production (flag wins)
CUENV_ENV=development cuenv exec -- echo $PORT  # Uses development
```
