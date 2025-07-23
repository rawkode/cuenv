# cuenv

[![CI](https://github.com/korora-tech/cuenv/workflows/CI/badge.svg)](https://github.com/korora-tech/cuenv/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/cuenv.svg)](https://crates.io/crates/cuenv)

A direnv alternative that uses CUE files for environment configuration.

## Prerequisites

cuenv requires Go to be installed for building the libcue bindings:

```bash
# Install Go if not already installed
# See https://golang.org/doc/install
```

## Installation

```bash
cargo install --path .
```

## Setup

Add the following to your shell configuration:

### Bash (~/.bashrc)
```bash
eval "$(cuenv init bash)"
```

### Zsh (~/.zshrc)
```zsh
eval "$(cuenv init zsh)"
```

### Fish (~/.config/fish/config.fish)
```fish
cuenv init fish | source
```

## Usage

1. Create an `env.cue` file in your project directory:

```cue
package env

DATABASE_URL: "postgres://localhost/mydb"
API_KEY: "secret123"
DEBUG: true
PORT: 3000
```

2. Navigate to the directory and the environment will be automatically loaded.

## Commands

- `cuenv` - Check if env.cue exists in current directory
- `cuenv load [directory]` - Manually load environment from a directory
- `cuenv unload` - Unload the current environment
- `cuenv status` - Show environment changes
- `cuenv hook <shell>` - Generate shell-specific hook output
- `cuenv init <shell>` - Generate shell initialization script
- `cuenv run <command> [args...]` - Run a command in a hermetic environment with only CUE-defined variables

## Features

- Automatic environment loading when entering directories
- Hierarchical environment loading (loads parent env.cue files first)
- Shell variable expansion support
- Support for multiple shells (bash, zsh, fish)
- Type-safe configuration with CUE
- Secret resolution from 1Password and GCP Secrets Manager (with `cuenv run`)
- Automatic secret obfuscation in stdout/stderr to prevent accidental exposure
- Environment-specific configurations with inheritance
- Capability-based variable filtering for secure credential management
- Command inference for automatic capability detection
- Environment variable configuration (CUENV_ENV, CUENV_CAPABILITIES)

## CUE File Format

Your `env.cue` files must declare `package env` and can contain string, number, or boolean values:

```cue
package env

// String values
DATABASE_URL: "postgres://user:pass@host/db"

// Number values  
PORT: 3000
TIMEOUT: 30

// Boolean values
DEBUG: true
ENABLE_CACHE: false

// Shell expansion is supported
LOG_PATH: "$HOME/logs/myapp"

// CUE features are supported
BASE_URL: "https://api.example.com"
API_ENDPOINT: "\(BASE_URL)/v1"  // String interpolation
DEBUG_PORT: PORT + 1            // Computed values
ENVIRONMENT: *"development" | string  // Defaults
```

## How It Works

1. When you cd into a directory, cuenv checks for `env.cue` files
2. It loads all env.cue files from the root directory to the current directory
3. Environment variables are set in your shell
4. When you leave the directory, the environment is restored

## Running Commands in Hermetic Environment

The `run` command executes programs with only the environment variables defined in your CUE files (plus PATH and HOME for basic functionality):

```bash
# Run a command with CUE-defined environment
cuenv run node server.js

# Pass arguments to the command
cuenv run npm -- install --save-dev

# Run shell commands
cuenv run bash -- -c "echo PORT=\$PORT"

# The environment is hermetic - parent environment variables are not passed through
export PARENT_VAR=123
cuenv run bash -- -c 'echo "PARENT_VAR=$PARENT_VAR"'  # Will print: PARENT_VAR=
```

### Secret Resolution

When using `cuenv run`, secret references in your CUE files are automatically resolved:

```cue
package env

// Regular environment variables
DATABASE_HOST: "localhost"
DATABASE_USER: "myapp"

// Secret references - both formats supported

// String format (traditional)
DATABASE_PASSWORD: "op://Personal/database/password"
STRIPE_SECRET_KEY: "gcp-secret://my-project/stripe-api-key"

// Structured format (type-safe)
#OnePasswordRef: { ref: string }
API_KEY: #OnePasswordRef & { ref: "op://Work/myapp-api-key" }

#GcpSecret: { project: string, secret: string, version?: string }
JWT_SECRET: #GcpSecret & {
    project: "prod-project"
    secret: "jwt-signing-key"
    version: "latest"
}

// You can compose URLs with resolved secrets
DATABASE_URL: "postgres://\(DATABASE_USER):\(DATABASE_PASSWORD)@\(DATABASE_HOST):5432/myapp"
```

**Requirements:**
- For 1Password: Install [1Password CLI](https://developer.1password.com/docs/cli/) and authenticate with `op signin`
- For GCP Secrets: Install [gcloud CLI](https://cloud.google.com/sdk/docs/install) and authenticate with `gcloud auth login`

**Note:** Secret resolution only happens with `cuenv run`. Regular `cuenv load` will not resolve secrets for security reasons.

#### Secret Obfuscation

When using `cuenv run`, any resolved secret values are automatically obfuscated in the command's stdout and stderr output. This prevents accidental exposure of sensitive information in logs or terminal output.

```bash
# Example: If DATABASE_PASSWORD resolves to "secret123"
cuenv run sh -c 'echo "Password is: $DATABASE_PASSWORD"'
# Output: Password is: ***********

# Secrets are obfuscated even in error messages
cuenv run sh -c 'echo "Error: Failed to connect with $API_KEY" >&2'
# Stderr: Error: Failed to connect with ***********
```

This obfuscation applies to all resolved secrets from 1Password and GCP Secrets Manager, helping maintain security when running commands with sensitive data.

### Environments and Capabilities

cuenv supports environment-specific configurations and capability-based filtering:

```cue
package env

// Base configuration
DATABASE_URL: "postgresql://localhost:5432/myapp"
LOG_LEVEL: "info"

// Environment-specific overrides
environment: {
    production: {
        DATABASE_URL: "postgresql://prod-db:5432/myapp"
        LOG_LEVEL: "warn"
    }
}

// Capability-tagged variables
AWS_ACCESS_KEY: "key" @capability("aws")
AWS_SECRET_KEY: "secret" @capability("aws")

// Command mappings for automatic capability inference
Commands: {
    terraform: capabilities: ["aws", "cloudflare"]
    aws: capabilities: ["aws"]
}
```

Usage:
```bash
# Use production environment
cuenv run -e production -- ./app

# Enable specific capabilities
cuenv run -c aws -- aws s3 ls

# Use environment variables
CUENV_ENV=production CUENV_CAPABILITIES=aws cuenv run -- terraform apply

# Automatic capability inference from command
cuenv run -e production -- aws s3 ls  # Automatically enables 'aws' capability
```

## Documentation

- **[Usage Guide](docs/USAGE.md)** - Comprehensive usage documentation
- **[Commands Reference](docs/COMMANDS.md)** - Complete command reference
- **[Secret Management](docs/SECRETS.md)** - Secret management and security guide
- **[Structured Secrets](docs/STRUCTURED_SECRETS.md)** - Type-safe secret definitions with CUE
- **[Environment Variables](docs/ENVIRONMENT_VARIABLES.md)** - Using environment variables for configuration

## Differences from direnv

- Uses CUE instead of shell scripts for configuration
- Type-safe configuration files
- No need for `direnv allow` (can be added if needed)
- Simpler mental model - just key-value pairs in CUE format