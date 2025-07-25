---
title: Configuration Reference
description: Complete reference for cuenv configuration options
---

## Configuration Files

### env.cue

The primary configuration file that defines environment variables.

**Location:** Project root or any parent directory
**Required:** Yes (for environment loading)

```cue
package env

// Environment variables
KEY: "value"
```

### Configuration Hierarchy

cuenv loads configurations hierarchically from parent directories:

```
/home/user/            (env.cue: ORG="mycompany")
  projects/            (env.cue: TEAM="backend")
    myapp/             (env.cue: APP="myapp")
      src/             (inherits all above)
```

Result in `/home/user/projects/myapp/src/`:

- `ORG=mycompany`
- `TEAM=backend`
- `APP=myapp`

## Environment Variables

### cuenv Configuration

These variables control cuenv's behavior:

#### CUENV_FILE

Custom filename for environment configuration.

**Default:** `env.cue`
**Example:**

```bash
export CUENV_FILE="environment.cue"
```

#### CUENV_DEBUG

Enable debug output.

**Default:** `0` (disabled)
**Values:** `0`, `1`, `true`, `false`
**Example:**

```bash
export CUENV_DEBUG=1
```

#### CUENV_DISABLE_AUTO

Disable automatic environment loading.

**Default:** `0` (auto-loading enabled)
**Values:** `0`, `1`, `true`, `false`
**Example:**

```bash
export CUENV_DISABLE_AUTO=1
```

#### CUENV_ENV

Default environment for `cuenv run`.

**Default:** None (uses base configuration)
**Example:**

```bash
export CUENV_ENV=production
```

#### CUENV_CAPABILITIES

Default capabilities for `cuenv run`.

**Default:** None
**Format:** Comma-separated list
**Example:**

```bash
export CUENV_CAPABILITIES=aws,database
```

### Runtime Variables

These variables are set by cuenv during execution:

#### CUENV_DIR

The directory containing the currently loaded environment.

**Set when:** Environment is loaded
**Example:**

```bash
echo "Environment directory: $CUENV_DIR"
```

#### CUENV_FILE

The path to the currently loaded environment file.

**Set when:** Environment is loaded
**Example:**

```bash
echo "Loaded from: $CUENV_FILE"
```

#### CUENV_WATCHES

Colon-separated list of watched files for auto-reload.

**Set when:** Environment is loaded
**Format:** Colon-separated paths
**Example:**

```bash
# View watched files
echo "$CUENV_WATCHES" | tr ':' '\n'
```

#### CUENV_DIFF

Base64-encoded environment diff for restoration.

**Set when:** Environment is loaded
**Internal use:** For restoring previous environment state

#### CUENV_LOADED

Indicates an environment is currently loaded (legacy).

**Set when:** Environment is loaded
**Unset when:** Environment is unloaded
**Example usage:**

```bash
if [[ -n "$CUENV_LOADED" ]]; then
    echo "cuenv environment active"
fi
```

#### CUENV_ROOT

Path to the directory containing the loaded `env.cue` (legacy, same as CUENV_DIR).

**Set when:** Environment is loaded
**Example:**

```bash
echo "Environment loaded from: $CUENV_ROOT"
```

#### CUENV_PREV\_\*

Previous values of modified variables.

**Format:** `CUENV_PREV_<VARIABLE_NAME>`
**Example:**

```bash
# If PATH was modified
echo "Previous PATH: $CUENV_PREV_PATH"
```

## CUE Configuration

### Package Declaration

Every `env.cue` must declare package env:

```cue
package env

// Configuration follows...
```

### Variable Types

#### String Variables

```cue
package env

// Simple string
NAME: "value"

// Multi-line string
DESCRIPTION: """
    Multi-line
    text
    """

// String with interpolation
BASE: "hello"
MESSAGE: "\(BASE) world"
```

#### Numeric Variables

```cue
package env

// Integer
PORT: 3000

// Float (converted to string)
VERSION: 1.5

// Computed
BASE_PORT: 3000
DEBUG_PORT: BASE_PORT + 1
```

#### Boolean Variables

```cue
package env

// Converted to "true" or "false" strings
DEBUG: true
PRODUCTION: false
```

### Environment-Specific Configuration

Define environment overrides:

```cue
package env

// Base configuration
PORT: 3000
DEBUG: true

// Environment overrides
environment: {
    production: {
        PORT: 8080
        DEBUG: false
    }
    staging: {
        PORT: 3001
    }
}
```

### Capability Tagging

Tag variables with required capabilities:

```cue
package env

// Tagged variables
AWS_KEY: "key" @capability("aws")
DB_URL: "postgres://..." @capability("database")

// Multiple capabilities
ADMIN_TOKEN: "token" @capability("admin", "sensitive")
```

### Command Mapping

Map commands to capabilities:

```cue
package env

Commands: {
    terraform: capabilities: ["aws", "cloudflare"]
    aws: capabilities: ["aws"]
    psql: capabilities: ["database"]
}
```

## Secret References

### 1Password Format

```cue
package env

import "github.com/rawkode/cuenv/cue"

// Structured format (recommended)
PASSWORD: cuenv.#OnePasswordRef & {ref: "op://vault/item/field"}
API_KEY: cuenv.#OnePasswordRef & {ref: "op://vault/item/section/field"}

// Example with capability
SECRET: cuenv.#OnePasswordRef & {ref: "op://Personal/secret"} @capability("security")
```

### GCP Secrets Format

```cue
package env

// URL format
SECRET: "gcp-secret://project/secret-name"
KEY: "gcp-secret://project/secret-name/version"

// Structured format
#GcpSecret: {
    project: string
    secret: string
    version?: string
}
TOKEN: #GcpSecret & {
    project: "my-project"
    secret: "api-token"
}
```

## Shell Integration

### Bash Configuration

```bash title="~/.bashrc"
eval "$(cuenv init bash)"

# Custom prompt
PS1='[\u@\h \W$(cuenv_prompt)]\$ '
cuenv_prompt() {
    [[ -n "$CUENV_LOADED" ]] && echo " (cuenv)"
}
```

### Zsh Configuration

```zsh title="~/.zshrc"
eval "$(cuenv init zsh)"

# With Oh My Zsh
plugins=(... cuenv)
```

### Fish Configuration

```fish title="~/.config/fish/config.fish"
if command -v cuenv >/dev/null 2>&1
    cuenv init fish | source
end
```

## Advanced Patterns

### Conditional Configuration

```cue
package env

import "strings"

// Conditional based on other variables
_isProd: ENVIRONMENT == "production"

DEBUG: {
    if _isProd { false }
    if !_isProd { true }
}
```

### Variable Validation

```cue
package env

// Constrained values
PORT: int & >=1024 & <=65535
ENVIRONMENT: "dev" | "staging" | "prod"
LOG_LEVEL: *"info" | "debug" | "warn" | "error"
```

### Computed Configurations

```cue
package env

// Base values
REGION: "us-east-1"
SERVICE: "myapp"
STAGE: "prod"

// Computed
BUCKET_NAME: "\(SERVICE)-\(STAGE)-\(REGION)"
FUNCTION_NAME: "\(SERVICE)-\(STAGE)-handler"
```

### Nested Structures

```cue
package env

// Define structure
_config: {
    app: {
        name: "myapp"
        version: "1.0.0"
    }
    database: {
        host: "localhost"
        port: 5432
    }
}

// Export as flat variables
APP_NAME: _config.app.name
APP_VERSION: _config.app.version
DB_HOST: _config.database.host
DB_PORT: _config.database.port
```

## Platform-Specific Notes

### Linux

- Shell integration works with system shells
- Supports all major distributions
- Uses standard environment variable mechanisms

### macOS

- Works with both system and Homebrew shells
- Compatible with Terminal.app and iTerm2
- Supports macOS 10.15+

### Windows

- **Git Bash**: Full support
- **WSL**: Full support (works as Linux)
- **PowerShell**: Planned for future release
- **CMD**: Not supported

## Performance Tuning

### State Management

cuenv uses environment variables for state management, eliminating file I/O:

- No temporary files to create or clean up
- State stored in memory (environment variables)
- Instant loading and unloading
- Automatic file watching with minimal overhead

### XDG Compliance

cuenv follows XDG Base Directory specifications:

```bash
# State files (when needed)
~/.local/state/cuenv/     # Linux
~/Library/State/cuenv/    # macOS
%LOCALAPPDATA%\cuenv\     # Windows

# Cache files
~/.cache/cuenv/           # Linux
~/Library/Caches/cuenv/   # macOS
%LOCALAPPDATA%\cuenv\     # Windows
```

### Lazy Loading

For faster shell startup:

```bash
# Defer initialization
alias cuenv='command cuenv'
```

### Disable Features

```bash
# Disable auto-loading for performance
export CUENV_DISABLE_AUTO=1

# Manually load when needed
cd /project && cuenv load
```

## Security Considerations

### Secret Handling

- Secrets are only resolved with `cuenv run`
- Never logged or displayed in plain text
- Automatically obfuscated in output
- Not written to disk

### File Permissions

cuenv uses SHA256 hashing for secure approval:

- Run `cuenv allow` once per directory
- SHA256 hash tracks file content changes
- Approved files reload automatically on changes
- More secure than path-based approval

Recommended file permissions:

```bash
# Restrict env.cue to user
chmod 600 env.cue

# Or to group
chmod 640 env.cue
```

### Capability Isolation

Use capabilities to limit exposure:

```cue
package env

// Only available with 'sensitive' capability
PRIVATE_KEY: "..." @capability("sensitive")
```

## Troubleshooting

### Common Issues

1. **Environment not loading**

   - Check file is named correctly
   - Verify package declaration
   - Enable debug mode

1. **Variables not set**

   - Check CUE syntax
   - Verify no validation errors
   - Look for typos in variable names

1. **Secrets not resolving**
   - Ensure secret manager is authenticated
   - Check secret reference format
   - Verify permissions

### Debug Commands

```bash
# Check CUE syntax
cue eval env.cue

# Test loading manually
CUENV_DEBUG=1 cuenv load

# Verify hook is working
cuenv hook bash
```
