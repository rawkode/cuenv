# Layered Hooks Test Example

This example demonstrates the new layered hook system in cuenv with support for:

- **#Exec hooks** - Basic command execution
- **#NixFlake hooks** - Nix flake integration with environment sourcing
- **Multiple hooks** - Arrays of hooks that execute in sequence

## Hook Configuration

```cue
hooks: {
  onEnter: [
    // Basic exec hook
    {
      command: "echo"
      args: ["🚀 Basic exec hook executed!"]
    },
    // Nix flake hook with environment sourcing
    {
      flake: {
        dir: "."
      }
      source: true
    }
  ]
}
```

## Features Demonstrated

1. **Multiple hook types**: Exec and NixFlake hooks in the same configuration
2. **Environment sourcing**: Nix flake environment variables are sourced and available to tasks
3. **Layered architecture**: Clean separation between execution primitives and orchestration
4. **Type safety**: CUE schema validation for all hook configurations

## Usage

```bash
# Allow the directory
cuenv allow .

# Run a task to see the merged environment
cuenv run test

# The environment will include:
# - CUE-defined variables (PROJECT_NAME, ENVIRONMENT)
# - Nix flake sourced variables (FLAKE_SHELL_ACTIVE, NODE_VERSION, etc.)
```