---
title: Environment Variables Reference
description: Complete list of environment variables used by cuenv
---

## Configuration Variables

These environment variables control cuenv's behavior.

### CUENV_FILE

Specifies a custom filename for environment configuration files.

- **Type:** String
- **Default:** `env.cue`
- **Examples:** `app.cue`, `config.cue`, `.env.cue`

```bash
# Use a different filename
export CUENV_FILE="config.cue"

# cuenv will now look for config.cue instead of env.cue
cd /path/to/project  # Loads from config.cue if present
```

### CUENV_ENV

Selects which environment configuration to load.

- **Type:** String
- **Default:** `default`
- **Examples:** `development`, `staging`, `production`

```bash
# Set environment
export CUENV_ENV="production"

# Or use with command
cuenv run -e staging -- npm start
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

## Runtime Variables

These variables are set by cuenv during operation.

### CUENV_ROOT

The directory containing the loaded environment file.

- **Type:** String (path)
- **Set by:** `cuenv load` when a file is found
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
cuenv load

# cuenv automatically sets:
# CUENV_PREV_DATABASE_URL="postgres://localhost/dev"

# Unload restores original value
cuenv unload
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

## Secret Management Variables

### CUENV_ONEPASSWORD_ACCOUNT

Default 1Password account for secret resolution.

- **Type:** String
- **Example:** `my-team.1password.com`

```bash
export CUENV_ONEPASSWORD_ACCOUNT="acme.1password.com"

# Now @1password tags use this account by default
# @1password(vault: "Production", item: "API Keys", field: "token")
```

### CUENV_ONEPASSWORD_VAULT

Default vault for 1Password references.

- **Type:** String
- **Example:** `Production`, `Development`

```bash
export CUENV_ONEPASSWORD_VAULT="Production"

# Simplified references:
# @1password(item: "Database", field: "password")
```

### CUENV_GCP_PROJECT

Default Google Cloud project for Secret Manager.

- **Type:** String
- **Example:** `my-project-123`

```bash
export CUENV_GCP_PROJECT="acme-prod"

# Simplified references:
# @gcp(secret: "api-key", version: "latest")
```

## Command-Specific Variables

### CUENV_RUN_COMMAND

Used internally by `cuenv run` to pass commands to subshells.

- **Type:** String
- **Internal use only**

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
# env.cue defines: environment: production: { PORT: 8080 }
# Shell has: export CUENV_ENV=staging

cuenv run -- echo $PORT                    # Uses staging
cuenv run -e production -- echo $PORT      # Uses production (flag wins)
CUENV_ENV=development cuenv run -- echo $PORT  # Uses development
```
