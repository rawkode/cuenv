# CUE Environment Examples

This directory contains examples demonstrating different features of cuenv.

## Structure

All examples use the `package env` declaration and define environment variables directly at the top level:

- **basic/** - Simple environment variables with CUE interpolation
- **with-capabilities/** - Capability-based variable filtering and commands
- **nested/** - Demonstrates directory hierarchy with parent/child configurations
- **hooks/** - Lifecycle hooks for onEnter and onExit events

## Usage

To use these examples:

```bash
# Load environment from a specific example
cd examples/basic
cuenv load

# Run a command with capabilities
cd examples/with-capabilities
cuenv run -c aws deploy

# Export for your shell
cd examples/basic
eval $(cuenv load)
```

## Basic Structure

The standard structure for a cuenv file is:

```cue
package env

// Environment variables
DATABASE_URL: "postgres://localhost/mydb"
API_KEY:      "your-api-key"

// Use CUE features
BASE_URL:     "https://api.example.com"
API_ENDPOINT: "\(BASE_URL)/v1"

// Add capabilities
AWS_REGION: "us-east-1" @capability("aws")

// Define commands with required capabilities
Commands: {
    deploy: {
        capabilities: ["aws", "docker"]
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
```

## Key Points

1. **Package Declaration**: Always use `package env`
2. **Top-level Variables**: Define environment variables directly at the top level
3. **No Import Required**: No need to import external packages for basic usage
4. **CUE Features**: String interpolation, constraints, and defaults work as expected
5. **Capabilities**: Use `@capability("name")` to tag variables
6. **Commands**: Define command capability mappings in the `Commands` object
7. **Environments**: Use the `environment` object for environment-specific overrides
8. **Hooks**: Use the `hooks` object for lifecycle events
