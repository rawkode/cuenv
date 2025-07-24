# CUE Environment Examples

This directory contains examples demonstrating different features of cuenv using the CUE package approach.

## Structure

All examples use the `package cuenv` declaration and can be evaluated using CUE's module system:

- **basic/** - Simple environment variables with CUE interpolation
- **with-capabilities/** - Capability-based variable filtering and commands
- **structured-secrets/** - Integration with 1Password using secret references
- **registry-secrets/** - Various secret manager integrations (GitHub, GitLab, AWS, etc.)
- **nested/** - Demonstrates directory hierarchy with parent/child configurations

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

## Key Changes from CUENV_FILE

Instead of using `CUENV_FILE` to specify individual files, cuenv now evaluates the entire CUE package in the current directory using `cue eval -p cuenv`. This enables:

1. **Module composition** - Import and compose CUE modules
1. **Type safety** - Use CUE's type system for validation
1. **Better organization** - Split configurations across multiple files
1. **Hierarchy support** - CUE's natural module resolution

## Writing Your Own Configuration

Create an `env.cue` file in your project directory:

```cue
package cuenv

// Your environment variables
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
```

Then run `cuenv load` or `cuenv run <command>` from that directory.
