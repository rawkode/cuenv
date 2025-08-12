# Preload Hooks Example

This example demonstrates the preload hook feature that allows long-running preparation tasks to execute in the background without blocking the shell when users `cd` into a directory.

## Features

- **Regular hooks** (`preload: false` or unset): Execute synchronously, blocking the shell
- **Preload hooks** (`preload: true`): Execute in background, shell remains responsive
- **Source hooks** (`source: true`): Always execute synchronously to capture environment variables

## Usage

1. Enter the directory:

   ```bash
   cd examples/hooks-preload
   ```

2. The shell returns immediately, allowing you to run commands like:

   ```bash
   ls          # Works immediately
   cat env.cue # Works immediately
   ```

3. When you run a cuenv command, it waits for preload hooks to complete:
   ```bash
   cuenv task test  # Waits for preload hooks, then runs
   cuenv exec echo "hello"  # Waits for preload hooks, then runs
   ```

## Hook Types in This Example

1. **Regular hook**: Prints a message synchronously when entering the directory
2. **Preload hooks**: Simulate a slow operation (5 second sleep) that runs in background
3. **Source hook**: Provides environment variables synchronously

## Real-World Use Cases

Replace the example sleep commands with actual slow operations:

```cue
hooks: onEnter: [
    // Preload Nix development environment
    {
        command: "nix"
        args: ["develop", "--command", "true"]
        preload: true
    },

    // Preload devenv shell
    {
        command: "devenv"
        args: ["shell", "dump"]
        preload: true
    },

    // Download dependencies in background
    {
        command: "npm"
        args: ["install"]
        preload: true
    }
]
```

## Benefits

- **Better UX**: No 30+ second wait when entering directories
- **Shell responsiveness**: Can browse files while environment prepares
- **Safety**: Commands that need the environment wait automatically
- **Backward compatible**: Existing hooks work unchanged
