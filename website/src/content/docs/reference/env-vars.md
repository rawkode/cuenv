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
- **Example:** `CUENV_FILE=environment.cue`

```bash
# Use custom filename
export CUENV_FILE="project.cue"

# Now cuenv looks for project.cue instead of env.cue
cd /my/project  # Loads from project.cue
```

### CUENV_DEBUG

Enables verbose debug output for troubleshooting.

- **Type:** Boolean (0/1, true/false)
- **Default:** `0`
- **Example:** `CUENV_DEBUG=1`

```bash
# Enable debug mode
export CUENV_DEBUG=1
cuenv load

# Output includes:
# - Files being checked
# - Parsing details
# - Variable resolution
# - Secret manager calls
```

### CUENV_DISABLE_AUTO

Disables automatic environment loading when changing directories.

- **Type:** Boolean (0/1, true/false)
- **Default:** `0`
- **Example:** `CUENV_DISABLE_AUTO=1`

```bash
# Disable auto-loading
export CUENV_DISABLE_AUTO=1

# Must manually load environments
cd /my/project
cuenv load  # Required
```

### CUENV_ENV

Sets the default environment for `cuenv run` commands.

- **Type:** String
- **Default:** None (uses base configuration)
- **Example:** `CUENV_ENV=production`

```bash
# Set default environment
export CUENV_ENV=production

# These are equivalent:
cuenv run -- node app.js
cuenv run -e production -- node app.js
```

### CUENV_CAPABILITIES

Sets default capabilities for `cuenv run` commands.

- **Type:** Comma-separated string
- **Default:** None
- **Example:** `CUENV_CAPABILITIES=aws,database`

```bash
# Set default capabilities
export CUENV_CAPABILITIES=aws,database

# These are equivalent:
cuenv run -- terraform plan
cuenv run -c aws,database -- terraform plan
```

## Runtime Variables

These variables are set by cuenv during execution.

### CUENV_LOADED

Indicates whether a cuenv environment is currently loaded.

- **Type:** String (path to env.cue)
- **Set by:** `cuenv load` or automatic loading
- **Unset by:** `cuenv unload` or leaving directory

```bash
# Check if environment is loaded
if [[ -n "$CUENV_LOADED" ]]; then
    echo "cuenv environment is active"
fi

# Use in prompt
PS1='$([ -n "$CUENV_LOADED" ] && echo "(cuenv) ")$ '
```

### CUENV_ROOT

Path to the directory containing the currently loaded env.cue file.

- **Type:** String (absolute path)
- **Set by:** `cuenv load`
- **Unset by:** `cuenv unload`

```bash
# Show where environment was loaded from
echo "Environment loaded from: $CUENV_ROOT"

# Use in scripts
if [[ "$CUENV_ROOT" == "$HOME/projects/myapp" ]]; then
    echo "In myapp project"
fi
```

### CUENV_PREV_*

Stores previous values of environment variables that were modified.

- **Type:** String
- **Format:** `CUENV_PREV_<VARIABLE_NAME>`
- **Set by:** `cuenv load` (for modified variables)
- **Used by:** `cuenv unload` (to restore values)

```bash
# If env.cue modifies PATH
echo $CUENV_PREV_PATH  # Shows original PATH value

# Check what was modified
env | grep '^CUENV_PREV_' | cut -d_ -f3- | cut -d= -f1
```

## Shell Integration Variables

### PROMPT_COMMAND (Bash)

Modified by cuenv to include directory change detection.

```bash
# cuenv adds its hook to PROMPT_COMMAND
echo $PROMPT_COMMAND
# Output includes: _cuenv_hook
```

### precmd_functions (Zsh)

Array of functions called before each prompt.

```zsh
# cuenv adds its hook
echo $precmd_functions
# Output includes: _cuenv_hook
```

## Secret Manager Variables

### 1Password Variables

Used by 1Password CLI integration:

- `OP_SESSION_*` - Authentication tokens
- `OP_DEVICE` - Device UUID
- `OP_CACHE` - Cache directory

### GCP Variables

Used by Google Cloud SDK:

- `GOOGLE_APPLICATION_CREDENTIALS` - Service account key
- `CLOUDSDK_CORE_PROJECT` - Default project
- `CLOUDSDK_CONFIG` - Config directory

## Usage Examples

### Development Setup

```bash
# ~/.bashrc or ~/.zshrc

# Development defaults
export CUENV_ENV=development
export CUENV_DEBUG=0
export CUENV_CAPABILITIES=database

# Production overrides
alias prod='CUENV_ENV=production cuenv run'
alias staging='CUENV_ENV=staging cuenv run'
```

### CI/CD Environment

```yaml
# .github/workflows/deploy.yml
env:
  CUENV_ENV: ${{ github.ref == 'refs/heads/main' && 'production' || 'staging' }}
  CUENV_CAPABILITIES: aws,database,deploy
  CUENV_DEBUG: ${{ runner.debug }}
```

### Docker Configuration

```dockerfile
# Dockerfile
ENV CUENV_DISABLE_AUTO=1
ENV CUENV_ENV=production
ENV CUENV_CAPABILITIES=database,redis
```

### Debugging Script

```bash
#!/bin/bash
# debug-cuenv.sh

echo "=== cuenv Environment Variables ==="
echo "CUENV_FILE: ${CUENV_FILE:-env.cue}"
echo "CUENV_DEBUG: ${CUENV_DEBUG:-0}"
echo "CUENV_DISABLE_AUTO: ${CUENV_DISABLE_AUTO:-0}"
echo "CUENV_ENV: ${CUENV_ENV:-<none>}"
echo "CUENV_CAPABILITIES: ${CUENV_CAPABILITIES:-<none>}"
echo
echo "=== Runtime Variables ==="
echo "CUENV_LOADED: ${CUENV_LOADED:-<not loaded>}"
echo "CUENV_ROOT: ${CUENV_ROOT:-<not set>}"
echo
echo "=== Modified Variables ==="
env | grep '^CUENV_PREV_' | while read -r line; do
    var="${line%%=*}"
    original_var="${var#CUENV_PREV_}"
    echo "$original_var was modified"
done
```

## Best Practices

### 1. Environment-Specific Configs

```bash
# Local development
export CUENV_ENV=development
export CUENV_DEBUG=1

# CI/CD
export CUENV_ENV=ci
export CUENV_CAPABILITIES=test,build

# Production
export CUENV_ENV=production
export CUENV_CAPABILITIES=deploy,monitor
```

### 2. Project-Specific Settings

```bash
# In project directory
cat > .envrc << 'EOF'
export CUENV_FILE=environment.cue
export CUENV_CAPABILITIES=aws,terraform
EOF
```

### 3. Team Conventions

```bash
# Team ~/.bashrc template
export CUENV_DEBUG="${CUENV_DEBUG:-0}"
export CUENV_ENV="${CUENV_ENV:-development}"

# Aliases for common operations
alias ce='cuenv'
alias cer='cuenv run --'
alias ces='cuenv status'
```

### 4. Security Practices

```bash
# Never export sensitive values
export CUENV_CAPABILITIES=public  # OK
export API_KEY=secret123          # WRONG!

# Use capabilities to limit exposure
export CUENV_CAPABILITIES="${CUENV_CAPABILITIES:-safe}"
```

## Precedence Rules

When multiple sources set the same variable:

1. Command-line flags (highest priority)
2. Environment variables
3. Configuration file values
4. Default values (lowest priority)

Example:

```bash
# env.cue defines: environment: production: { PORT: 8080 }
# Shell has: export CUENV_ENV=staging

cuenv run -- echo $PORT                    # Uses staging
cuenv run -e production -- echo $PORT      # Uses production (flag wins)
CUENV_ENV=development cuenv run -- echo $PORT  # Uses development
```