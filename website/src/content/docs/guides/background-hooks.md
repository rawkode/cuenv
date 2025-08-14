---
title: Background Source Hooks
description: Advanced guide to long-running source hooks and background execution
---

# Background Source Hooks

cuenv supports advanced hook patterns for handling long-running operations that need to provide environment variables to your shell. This guide covers background source hooks, preload operations, and environment capture.

## Overview

Background source hooks are useful for:

- **Slow initialization scripts** (e.g., downloading dependencies)
- **Authentication tokens** that take time to fetch
- **Database connection strings** from vault/secret managers
- **Any environment setup** that shouldn't block the shell

## How Background Hooks Work

1. **Source hooks** - Hooks marked with `source: true` that export environment variables
2. **Background execution** - Pressing 'b' to continue hooks in the background  
3. **Automatic environment capture** - Shell hook detects completed background hooks
4. **One-time sourcing** - Captured environment is only sourced once

## Configuration

### Basic Source Hook

```cue
package env

hooks: {
    onEnter: [
        {
            command: "fetch-auth-token.sh"
            source: true
            preload: true
        }
    ]
}
```

### Long-running Source Hook

```cue
package env

hooks: {
    onEnter: [
        {
            command: "sh"
            args: ["-c", """
                echo "Fetching environment from vault..."
                sleep 5  # Simulate slow operation
                echo "export VAULT_TOKEN=$(vault auth -method=aws)"
                echo "export DATABASE_URL=$(vault kv get -field=url secret/db)"
                """]
            source: true
            preload: true
        }
    ]
}
```

## Usage Workflow

### 1. Allow the Directory

When you enter a directory with background hooks:

```bash
cd /path/to/project
# cuenv detects hooks and prompts for execution
```

### 2. Background the Hooks

When hooks are running, you can background them:

```bash
# Press 'b' when prompted to background long-running hooks
# The hooks continue running in the background
```

### 3. Check Status

Monitor background hook progress:

```bash
cuenv env status --hooks

# Example output:
# Hooks Status: 1 running, 0 completed
# Background hooks: 1 active
```

### 4. Environment Capture

The shell integration automatically captures completed hooks:

```bash
# Your shell prompt will update when hooks complete
# Environment variables are automatically sourced
echo $VAULT_TOKEN  # Available after hook completion
```

## Shell Integration

### Setup Background Hook Support

Configure your shell to check for completed background hooks:

```bash
# In ~/.bashrc or ~/.zshrc
eval "$(cuenv shell init bash)"

# Or for more control:
function cuenv_check_hooks() {
    if [[ -n "$CUENV_DIR" ]]; then
        eval "$(cuenv shell hook bash)"
    fi
}

# Add to your prompt function
PROMPT_COMMAND="cuenv_check_hooks;$PROMPT_COMMAND"
```

### Manual Hook Execution

You can also manually run and source hooks:

```bash
# Run hooks and capture output
cuenv shell hook bash > /tmp/cuenv-env

# Source the captured environment
source /tmp/cuenv-env

# Or in one step
eval "$(cuenv shell hook bash)"
```

## Advanced Patterns

### Conditional Background Execution

```cue
package env

hooks: {
    onEnter: [
        {
            command: "check-auth.sh"
            source: true
            preload: true
            // Only run if not already authenticated
            condition: "test -z \"$AUTH_TOKEN\""
        }
    ]
}
```

### Multiple Source Hooks

```cue
package env

hooks: {
    onEnter: [
        {
            command: "fetch-aws-creds.sh"
            source: true
            preload: true
            description: "Fetching AWS credentials"
        },
        {
            command: "fetch-db-config.sh"
            source: true
            preload: true
            description: "Loading database configuration"
        }
    ]
}
```

### Hook Dependencies

```cue
package env

hooks: {
    onEnter: [
        {
            id: "auth"
            command: "authenticate.sh"
            source: true
            preload: true
        },
        {
            command: "fetch-user-data.sh"
            source: true
            preload: true
            dependsOn: ["auth"]
        }
    ]
}
```

## Environment Capture Details

### Capture Mechanism

Background hooks write their output to temporary files:

```bash
# Hook output is captured to:
/tmp/cuenv-$USER/hooks-$DIR_HASH/source-output

# Shell integration checks this file and sources it once
```

### Output Format

Source hooks should output valid shell export statements:

```bash
#!/bin/bash
# fetch-auth-token.sh

# Simulate slow token fetch
sleep 3

# Output environment variables
echo "export AUTH_TOKEN=$(get-token)"
echo "export AUTH_EXPIRES=$(date -d '+1 hour' +%s)"
echo "export AUTH_USER=$(get-user)"
```

### Error Handling

```bash
#!/bin/bash
# robust-auth-hook.sh

set -e  # Exit on error

if ! command -v vault >/dev/null; then
    echo "# vault command not found" >&2
    exit 1
fi

# Try to authenticate
if token=$(vault auth -method=aws 2>/dev/null); then
    echo "export VAULT_TOKEN=$token"
else
    echo "# vault authentication failed" >&2
    exit 1
fi
```

## Monitoring and Debugging

### Check Hook Status

```bash
# View all hook activity
cuenv env status --hooks --verbose

# JSON output for scripting
cuenv env status --hooks --format=json
```

### Hook Logs

```bash
# Enable debug logging
RUST_LOG=debug cuenv env allow

# View hook execution logs
tail -f ~/.local/share/cuenv/logs/hooks.log
```

### Background Process Management

```bash
# List background hook processes
ps aux | grep cuenv

# Kill stuck background hooks
cuenv env deny  # Cleans up background processes
```

## Best Practices

### 1. Use Timeouts

```cue
hooks: {
    onEnter: [
        {
            command: "timeout"
            args: ["30", "slow-operation.sh"]
            source: true
            preload: true
        }
    ]
}
```

### 2. Provide User Feedback

```cue
hooks: {
    onEnter: [
        {
            command: "fetch-secrets.sh"
            source: true
            preload: true
            description: "Fetching secrets from vault (may take 10-15s)"
        }
    ]
}
```

### 3. Handle Failures Gracefully

```bash
#!/bin/bash
# fault-tolerant-hook.sh

# Try primary method
if result=$(primary-auth-method 2>/dev/null); then
    echo "export AUTH_TOKEN=$result"
    echo "export AUTH_METHOD=primary"
# Fallback to secondary method
elif result=$(secondary-auth-method 2>/dev/null); then
    echo "export AUTH_TOKEN=$result"
    echo "export AUTH_METHOD=secondary"
else
    # Set safe defaults
    echo "export AUTH_TOKEN="
    echo "export AUTH_METHOD=none"
    echo "# Authentication failed, using anonymous access" >&2
fi
```

### 4. Cache Results

```bash
#!/bin/bash
# cached-auth-hook.sh

CACHE_FILE="/tmp/cuenv-auth-cache-$USER"
CACHE_TTL=3600  # 1 hour

# Check if cache is still valid
if [[ -f "$CACHE_FILE" ]] && [[ $(($(date +%s) - $(stat -c %Y "$CACHE_FILE"))) -lt $CACHE_TTL ]]; then
    cat "$CACHE_FILE"
    exit 0
fi

# Fetch new token and cache it
token=$(fetch-new-token)
echo "export AUTH_TOKEN=$token" | tee "$CACHE_FILE"
```

## Use Cases

### Development Environment Setup

```cue
package env

hooks: {
    onEnter: [
        {
            command: "dev-setup.sh"
            source: true
            preload: true
            description: "Setting up development environment"
        }
    ]
}
```

Where `dev-setup.sh` might:
- Download dependencies
- Start background services
- Fetch development certificates
- Configure database connections

### CI/CD Integration

```cue
package env

hooks: {
    onEnter: [
        {
            command: "ci-auth.sh"
            source: true
            preload: true
            condition: "test -n \"$CI\""
        }
    ]
}
```

### Multi-Cloud Authentication

```cue
package env

hooks: {
    onEnter: [
        {
            id: "aws"
            command: "aws-auth.sh"
            source: true
            preload: true
        },
        {
            id: "gcp"
            command: "gcp-auth.sh"
            source: true
            preload: true
        },
        {
            command: "setup-cloud-configs.sh"
            source: true
            preload: true
            dependsOn: ["aws", "gcp"]
        }
    ]
}
```

## Related Guides

- [Shell Integration](/guides/shell-integration/) - Basic shell setup
- [Security](/guides/security/) - Security considerations for hooks
- [Hooks Reference](/reference/hooks/) - Complete hook configuration reference