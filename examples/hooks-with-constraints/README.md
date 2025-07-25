# Hooks with Constraints Example

This example demonstrates how to use constraints with hooks in cuenv. Constraints allow you to conditionally execute hooks based on the end user's environment, ensuring hooks only run when required tools are available.

## What this example shows

### Hook Constraints Types

1. **Command Exists** - Check if a command is available in PATH
2. **Shell Command** - Run arbitrary commands and check their exit code

### Example Scenarios

1. **devenv Integration** - The `onEnter` hook will only run `devenv up` if:
   - The `devenv` command is available in PATH

2. **Conditional Cleanup** - The `onExit` hook will only run cleanup if:
   - The `echo` command is available (demonstrating command checking)

## Testing the Example

### Test 1: Without devenv installed

```bash
cd examples/hooks-with-constraints
cuenv load
```

Expected behavior: The environment loads normally, but the `devenv up` hook is skipped because `devenv` is not installed.

### Test 2: With all constraints met

```bash
cd examples/hooks-with-constraints
# Mock the devenv command for testing
echo '#!/bin/bash\necho "devenv up executed"' > /tmp/devenv
chmod +x /tmp/devenv
PATH="/tmp:$PATH" cuenv load
```

Expected behavior: All constraints are met, so the `devenv up` hook executes.

### Test 3: Custom tool checking

```bash
cd examples/hooks-with-constraints
# Demonstrate shell command constraint
cuenv load
```

Expected behavior: The cleanup hook runs because `echo` is available on most systems.

## Constraint Configuration

Constraints are defined as an array in the hook configuration:

```cue
hooks: {
    onEnter: {
        command: "your-command"
        args: ["arg1", "arg2"]
        constraints: [
            {
                commandExists: {
                    command: "required-tool"
                }
            },
            {
                shellCommand: {
                    command: "nix"
                    args: ["--version"]
                }
            }
        ]
    }
}
```

All constraints must pass for the hook to execute. If any constraint fails, the hook is skipped with a log message.

## Design Philosophy

Constraints focus on checking the end user's environment for required tools rather than checking files or environment variables that are already defined within the cuenv environment. This ensures that hooks only run when the necessary external tools are available.