# cuenv

[![CI](https://github.com/rawkode/cuenv/workflows/ci/badge.svg)](https://github.com/rawkode/cuenv/actions)
[![codecov](https://codecov.io/gh/rawkode/cuenv/graph/badge.svg)](https://codecov.io/gh/rawkode/cuenv)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/cuenv.svg)](https://crates.io/crates/cuenv)

A direnv alternative that uses CUE packages for environment configuration.

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

#### Home Manager Module

If you're using [Home Manager](https://github.com/nix-community/home-manager), you can use the included module:

```nix
# In your flake.nix
{
  inputs = {
    cuenv.url = "github:rawkode/cuenv";
    # ... other inputs
  };

  outputs = { self, nixpkgs, home-manager, cuenv, ... }: {
    homeConfigurations.yourUsername = home-manager.lib.homeManagerConfiguration {
      # ... your configuration
      modules = [
        cuenv.homeManagerModules.default
        {
          programs.cuenv = {
            enable = true;
            # Optional: specify package
            # package = cuenv.packages.${pkgs.system}.default;

            # Shell integrations (auto-detected based on enabled shells)
            # enableBashIntegration = true;
            # enableZshIntegration = true;
            # enableFishIntegration = true;
            # enableNushellIntegration = true;  # Experimental
          };
        }
      ];
    };
  };
}
```

The module will automatically:

- Install the cuenv package
- Set up shell integration for enabled shells (bash, zsh, fish, nushell)
- Configure the shell hooks to load CUE environments automatically

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

1. Create a CUE package in your project directory with `.cue` files:

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

- `cuenv` - Load CUE package from current directory
- `cuenv load [directory]` - Manually load environment from a directory
- `cuenv unload` - Unload the current environment
- `cuenv status` - Show environment changes
- `cuenv hook <shell>` - Generate shell-specific hook output
- `cuenv init <shell>` - Generate shell initialization script
- `cuenv run <command> [args...]` - Run a command in a hermetic environment with only CUE-defined variables

## Features

- Automatic environment loading when entering directories
- CUE package-based environment loading
- Shell variable expansion support
- Support for multiple shells (bash, zsh, fish)
- Type-safe configuration with CUE
- Secret resolution from 1Password and GCP Secrets Manager (with `cuenv run`)
- Automatic secret obfuscation in stdout/stderr to prevent accidental exposure
- Environment-specific configurations with inheritance
- Capability-based variable filtering for secure credential management
- Command inference for automatic capability detection
- Environment variable configuration (CUENV_ENV, CUENV_CAPABILITIES)
- Monorepo support with cross-package task dependencies
- Task execution with automatic dependency resolution
- Staged dependency isolation for reproducible builds

## CUE File Format

Your CUE package should use the cuenv package schema:

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

1. When you cd into a directory, cuenv checks for CUE packages (directories with `.cue` files)
1. It loads the CUE package from the current directory
1. Environment variables are set in your shell
1. When you leave the directory, the environment is restored

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

### Access Restrictions

You can configure disk and network access restrictions for tasks using the `security` section in your CUE task definitions. This uses Landlock (Linux Security Module) for enforcement:

```cue
tasks: {
  "secure-build": {
    description: "Build the project with restricted filesystem access"
    command:     "echo 'Building project securely...' && sleep 1 && echo 'Build complete!'"
    security: {
      restrictDisk: true
      readOnlyPaths: ["/usr", "/lib", "/bin"]
      readWritePaths: ["/tmp", "./build"]
    }
  }
  "network-task": {
    description: "Task that needs network access but with restrictions"
    command:     "echo 'Downloading dependencies...' && curl --version"
    security: {
      restrictNetwork: true
      allowedHosts: ["api.example.com", "registry.npmjs.org"]
    }
  }
  "fully-restricted": {
    description: "Task with both disk and network restrictions"
    command:     "echo 'Running in secure sandbox'"
    security: {
      restrictDisk: true
      restrictNetwork: true
      readOnlyPaths: ["/usr/bin", "/bin"]
      readWritePaths: ["/tmp"]
      allowedHosts: ["localhost"]
    }
  }
  "unrestricted": {
    description: "Task without security restrictions"
    command:     "echo 'Running without restrictions' && ls -la /"
  }
}
```

**Running tasks with security restrictions:**

```bash
# Run a task with disk restrictions
cuenv run secure-build

# Run a task with network restrictions
cuenv run network-task

# Run a fully restricted task
cuenv run fully-restricted
```

**Landlock Requirements:**

- Linux kernel 5.13+ (for filesystem restrictions)
- Linux kernel 5.19+ (for network restrictions - basic support)
- Appropriate permissions to use Landlock LSM

**Security Configuration Options:**

- `restrictDisk`: Enable filesystem access restrictions
- `restrictNetwork`: Enable network access restrictions
- `readOnlyPaths`: Array of paths allowed for reading
- `readWritePaths`: Array of paths allowed for reading and writing
- `denyPaths`: Array of paths explicitly denied (overrides allow lists)
- `allowedHosts`: Array of network hosts/CIDRs allowed for connections

**Security Model:** When disk restrictions are enabled, you must explicitly allow all paths your task needs access to. This includes:

- Executable paths (`/bin`, `/usr/bin`)
- Library paths (`/lib`, `/usr/lib`, `/lib64`, `/usr/lib64`)
- Configuration paths (`/etc` if needed)
- Working directories and output paths

**Note:** Network restrictions are currently limited by Landlock V2 capabilities. The implementation will be enhanced in future versions to provide more granular network access control.

**Note:** Network and process restrictions are not yet fully implemented with Landlock. Use system-level controls or container runtimes for those restrictions.

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
    DATABASE_PASSWORD: cuenv.#OnePasswordRef & {ref: "op://Personal/database/password"}
    API_KEY: cuenv.#OnePasswordRef & {ref: "op://Work/myapp-api-key/field"}

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

    // Capability mappings for automatic inference
    capabilities: {
        aws: {
            commands: ["terraform", "aws", "deploy"]
        }
        cloudflare: {
            commands: ["terraform"]
        }
        docker: {
            commands: ["deploy"]
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
- **[Monorepo Support](docs/monorepo.md)** - Managing monorepo environments and cross-package tasks
- **[Configuration](https://cuenv.dev/reference/configuration)** - Configuration options reference
- **[Environment Variables](https://cuenv.dev/reference/env-vars)** - Using environment variables for configuration

## Differences from direnv

- Uses CUE instead of shell scripts for configuration
- Type-safe configuration files
- No need for `direnv allow` (can be added if needed)
- Simpler mental model - just key-value pairs in CUE format
