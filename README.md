# cuenv

[![CI](https://github.com/korora-tech/cuenv/workflows/CI/badge.svg)](https://github.com/korora-tech/cuenv/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/cuenv.svg)](https://crates.io/crates/cuenv)

A direnv alternative that uses CUE files for environment configuration.

## Development

Development is only supported through the Nix flake:

```bash
# Enter the development shell
nix develop

# Build the project
cargo build
```

## Installation

### Using Nix (Recommended)

```bash
# Install directly from the GitHub repository
nix profile install github:rawkode/cuenv

# Or run without installing
nix run github:rawkode/cuenv -- --help

# Using a specific version/commit
nix profile install github:rawkode/cuenv/<commit-sha>
```

### Using Cargo

```bash
# Install from crates.io
cargo install cuenv
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/rawkode/cuenv
cd cuenv

# Build with Nix
nix build

# The binary will be available in ./result/bin/cuenv
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

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
    DEBUG: "true"
    PORT: "3000"
}
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

Your `env.cue` files should use the cuenv package schema:

```cue
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // String values  
    DATABASE_URL: "postgres://user:pass@host/db"

    // String representations of numbers
    PORT: "3000"
    TIMEOUT: "30"

    // String representations of booleans
    DEBUG: "true"
    ENABLE_CACHE: "false"

    // Shell expansion is supported
    LOG_PATH: "$HOME/logs/myapp"

    // CUE features are supported
    BASE_URL: "https://api.example.com"
    API_ENDPOINT: "\(BASE_URL)/v1"  // String interpolation
    HOST: "localhost"
    DATABASE_DSN: "postgres://\(HOST):5432/myapp"  // Computed values
}
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

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Regular environment variables
    DATABASE_HOST: "localhost"
    DATABASE_USER: "myapp"

    // Secret references - 1Password format
    DATABASE_PASSWORD: "op://Personal/database/password"
    API_KEY: "op://Work/myapp-api-key/field"
    
    // Secret references - Various providers
    GITHUB_TOKEN: "github://myorg/myrepo/GITHUB_TOKEN"
    AWS_SECRET: "aws-secret://prod/api/secret"
    GCP_SECRET: "gcp-secret://myproject/db-password"
    AZURE_KEY: "azure-keyvault://myvault/keys/mykey"
    VAULT_TOKEN: "vault://secret/data/myapp/token"

    // You can compose URLs with resolved secrets
    DB_HOST: "prod.example.com"
    DATABASE_URL: "postgres://\(DATABASE_USER):\(DATABASE_PASSWORD)@\(DB_HOST):5432/myapp"
}
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

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Base configuration
    DATABASE_URL: "postgresql://localhost:5432/myapp"
    LOG_LEVEL: "info"
    PORT: "3000"

    // AWS capabilities - tagged with @capability
    AWS_REGION: "us-east-1" @capability("aws")
    AWS_ACCESS_KEY: "aws-access-key" @capability("aws")
    AWS_SECRET_KEY: "aws-secret-key" @capability("aws")

    // Docker capabilities
    DOCKER_REGISTRY: "docker.io" @capability("docker")
    DOCKER_IMAGE: "myapp:latest" @capability("docker")

    // Environment-specific overrides
    environment: {
        production: {
            DATABASE_URL: "postgresql://prod-db:5432/myapp"
            LOG_LEVEL: "warn"
            PORT: "8080"
            AWS_REGION: "us-west-2" @capability("aws")
        }
        staging: {
            DATABASE_URL: "postgresql://staging-db:5432/myapp"
            LOG_LEVEL: "debug"
        }
    }

    // Command mappings for automatic capability inference
    Commands: {
        terraform: {
            capabilities: ["aws", "cloudflare"]
        }
        aws: {
            capabilities: ["aws"]
        }
        deploy: {
            capabilities: ["aws", "docker"]
        }
    }
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

- **[Quickstart Guide](https://cuenv.dev/quickstart)** - Get started quickly with cuenv
- **[Commands Reference](https://cuenv.dev/reference/commands)** - Complete command reference
- **[Secret Management](https://cuenv.dev/guides/secrets)** - Secret management and security guide
- **[CUE Format Guide](https://cuenv.dev/guides/cue-format)** - Type-safe configuration with CUE
- **[Environments](https://cuenv.dev/guides/environments)** - Environment-specific configurations
- **[Capabilities](https://cuenv.dev/guides/capabilities)** - Capability-based variable filtering
- **[Shell Integration](https://cuenv.dev/guides/shell-integration)** - Setting up shell hooks
- **[Configuration](https://cuenv.dev/reference/configuration)** - Configuration options reference
- **[Environment Variables](https://cuenv.dev/reference/env-vars)** - Using environment variables for configuration

## Differences from direnv

- Uses CUE instead of shell scripts for configuration
- Type-safe configuration files
- No need for `direnv allow` (can be added if needed)
- Simpler mental model - just key-value pairs in CUE format
