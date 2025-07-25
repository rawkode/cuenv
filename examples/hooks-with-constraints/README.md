# Hooks with Constraints Example

This example demonstrates how to use constraints with hooks in cuenv. Constraints allow you to conditionally execute hooks based on system state, ensuring hooks only run when appropriate conditions are met.

## What this example shows

### Hook Constraints Types

1. **Command Exists** - Check if a command is available in PATH
2. **File Exists** - Check if files or directories exist  
3. **Environment Variable Set** - Check if environment variables are set
4. **Environment Variable Equals** - Check if environment variables equal specific values
5. **Shell Command** - Run arbitrary commands and check their exit code

### Example Scenarios

1. **devenv Integration** - The `onEnter` hook will only run `devenv up` if:
   - The `devenv` command is available in PATH
   - A `devenv.nix` file exists in the current directory

2. **Conditional Cleanup** - The `onExit` hook will only run cleanup if:
   - The `CLEANUP_MODE` environment variable is set to "auto"

## Testing the Example

### Test 1: Without devenv installed

```bash
cd examples/hooks-with-constraints
cuenv load
```

Expected behavior: The environment loads normally, but the `devenv up` hook is skipped because `devenv` is not installed.

### Test 2: With cleanup disabled

```bash
cd examples/hooks-with-constraints  
CLEANUP_MODE=manual cuenv load
# ... do some work ...
cuenv unload
```

Expected behavior: The cleanup hook is skipped on exit because `CLEANUP_MODE` is not "auto".

### Test 3: With all constraints met

```bash
cd examples/hooks-with-constraints
# Create a dummy devenv.nix file to satisfy the file constraint
touch devenv.nix
# Mock the devenv command for testing
echo '#!/bin/bash\necho "devenv up executed"' > /tmp/devenv
chmod +x /tmp/devenv
PATH="/tmp:$PATH" cuenv load
```

Expected behavior: All constraints are met, so the `devenv up` hook executes.

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
                fileExists: {
                    path: "required-file.txt"
                }
            },
            {
                envVarSet: {
                    var: "REQUIRED_VAR"
                }
            },
            {
                envVarEquals: {
                    var: "MODE"
                    value: "development"
                }
            },
            {
                shellCommand: {
                    command: "test"
                    args: ["-f", "/some/file"]
                }
            }
        ]
    }
}
```

All constraints must pass for the hook to execute. If any constraint fails, the hook is skipped with a log message.