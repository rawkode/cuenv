---
title: Example Configurations
description: Comprehensive examples demonstrating cuenv features using CUE configurations
---

# CUE Environment Examples

This guide provides examples demonstrating different features of cuenv using the CUE package approach.

## Structure

All examples use the `package examples` declaration and import the schema:

- **basic/** - Simple environment variables with CUE interpolation
- **with-capabilities/** - Capability-based variable filtering and commands
- **nested/** - Demonstrates directory hierarchy with parent/child configurations
- **hooks/** - Lifecycle hooks for onEnter and onExit events
- **custom-secrets/** - Custom command-based secret resolvers for various secret management systems

## Usage

To use these examples:

```bash
# Load environment from a specific example
cd examples/basic
cuenv shell load

# Run a command with capabilities
cd examples/with-capabilities
cuenv exec -c aws deploy

# Export for your shell
cd examples/basic
eval $(cuenv env export)
```

## Basic Structure

The standard structure for a cuenv file is:

```cue
package env

import "github.com/rawkode/cuenv"

// Environment configuration
env: cuenv.#Env & {
    // Environment variables
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY:      "your-api-key"

    // Use CUE features
    BASE_URL:     "https://api.example.com"
    API_ENDPOINT: "\(BASE_URL)/v1"

    // Add capabilities
    AWS_REGION: "us-east-1" @capability("aws")

    // Define capabilities with associated commands
    capabilities: {
        aws: {
            commands: ["deploy"]
        }
        docker: {
            commands: ["deploy"]
        }
    }

    // Environment-specific overrides
    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
        }
    }

    // Lifecycle hooks
    hooks: {
        onEnter: {
            command: "echo"
            args: ["Environment loaded"]
        }
    }
}
```

## Key Points

1. **Package Declaration**: Always use `package env`
2. **Import Schema**: Import `"github.com/rawkode/cuenv"` for the `#Env` schema
3. **Environment Block**: Define all configuration within `env: cuenv.#Env & { ... }`
4. **Type Safety**: The `#Env` schema provides validation and structure
5. **CUE Features**: String interpolation, constraints, and defaults work as expected
6. **Capabilities**: Use `@capability("name")` to tag variables
7. **Capabilities**: Define capability-to-command mappings in the `capabilities` object
8. **Environments**: Use the `environment` object for environment-specific overrides
9. **Hooks**: Use the `hooks` object for lifecycle events

## Advanced Examples

For more specific examples, see:

- [Security and Capabilities](/guides/capabilities/) - Capability-based access control
- [Secrets Management](/guides/secrets/) - Custom secret resolvers
- [Hooks and Lifecycle](/reference/hooks/) - Environment lifecycle management
- [Task Examples](/guides/task-examples/) - Task automation patterns
