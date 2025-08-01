---
title: devenv Integration
description: Seamlessly integrate cuenv with devenv for Nix-based development environments
---

# devenv Integration Example

This example shows how to use cuenv with devenv for seamless Nix development environment integration.

## How it works

1. **Environment Sourcing**: The `onEnter` hook runs `devenv print-dev-env` with `source: true`
2. **Variable Merging**: devenv's environment variables are merged with CUE-defined variables
3. **Precedence**: CUE variables (like `APP_ENV`, `DATABASE_URL`) override devenv variables
4. **Task Integration**: All cuenv tasks inherit the combined environment

## Usage

```bash
# Allow the directory
cuenv allow .

# Run tasks with devenv environment
cuenv run dev     # Start development server
cuenv run test    # Run tests
cuenv run build   # Build with full toolchain

# Execute commands with environment
cuenv exec node --version  # Uses devenv's Node.js
```

## Configuration Example

```cue
package env

import "github.com/rawkode/cuenv"

// Source devenv environment on entry
hooks: {
    onEnter: {
        command: "devenv"
        args: ["print-dev-env"]
        source: true
    }
}

// Define environment variables (override devenv)
env: cuenv.#Env & {
    APP_ENV: "development"
    DATABASE_URL: "postgres://localhost/myapp_dev"
    LOG_LEVEL: "debug"
}

// Define development tasks
tasks: {
    "dev": {
        description: "Start development server"
        command: "npm run dev"
    }

    "test": {
        description: "Run tests"
        command: "npm test"
    }

    "build": {
        description: "Build application"
        command: "npm run build"
        inputs: ["src/**", "package.json"]
        outputs: ["dist/**"]
    }
}
```

## devenv.nix Example

```nix
{ pkgs, ... }:

{
  # Development packages
  packages = with pkgs; [ nodejs_20 yarn python3 ];

  # Environment variables from devenv
  env = {
    NODE_ENV = "development";
    DEVENV_ACTIVE = "true";
  };

  # Scripts
  scripts.hello.exec = "echo 'Hello from devenv!'";
}
```

## Benefits

- ✅ **Type-safe configuration**: CUE schema validation for all environment setup
- ✅ **Environment precedence**: CUE variables override devenv when needed
- ✅ **Task definitions**: Structured task execution with dependencies
- ✅ **Multi-environment**: Different configs for dev/staging/prod
- ✅ **Automatic activation**: Environment loads automatically with tasks

## Comparison to .envrc

Instead of:

```bash
# .envrc
use devenv
export APP_ENV=development
export DATABASE_URL=postgres://localhost/myapp_dev
```

You get:

```cue
// env.cue - Type-safe, structured configuration
hooks: {
    onEnter: {
        command: "devenv"
        args: ["print-dev-env"]
        source: true
    }
}

env: cuenv.#Env & {
    APP_ENV: "development"
    DATABASE_URL: "postgres://localhost/myapp_dev"
}
```

## Advanced Integration

### Multiple Environments

```cue
env: cuenv.#Env & {
    BASE_URL: "http://localhost:3000"

    // Environment-specific overrides
    environment: {
        production: {
            BASE_URL: "https://api.production.com"
        }
        staging: {
            BASE_URL: "https://api.staging.com"
        }
    }
}
```

### Conditional devenv Activation

```cue
hooks: {
    onEnter: [
        // Only source devenv if devenv.nix exists
        if path.exists("devenv.nix") {
            command: "devenv"
            args: ["print-dev-env"]
            source: true
        },
        // Always run custom setup
        {
            command: "echo"
            args: ["Environment loaded with devenv integration"]
        }
    ]
}
```

## Related Guides

- [Nix Integration](/guides/nix-integration/) - Using nix develop
- [Hooks and Lifecycle](/reference/hooks/) - Environment lifecycle management
- [Task Examples](/guides/task-examples/) - Task automation patterns
