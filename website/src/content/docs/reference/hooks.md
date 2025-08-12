---
title: Hooks
description: Environment hooks and constraints for setup and teardown operations
---

# Hooks

Hooks allow you to run commands when entering or exiting a directory. They support background execution (preload), environment sourcing, and conditional execution based on constraints.

## Hook Types

### onEnter Hooks

Execute when entering a directory with an `env.cue` file.

### onExit Hooks

Execute when leaving a directory (changing to a different directory).

## Hook Execution Modes

### Regular Hooks (default)

Execute synchronously, blocking the shell until completion.

```cue
hooks: onEnter: [
    {
        command: "echo"
        args: ["Setting up environment"]
    }
]
```

### Preload Hooks

Execute in the background without blocking the shell. The shell remains responsive for commands like `ls` and `cat`, but `cuenv exec` and `cuenv task` commands wait for preload completion.

```cue
hooks: onEnter: [
    {
        command: "nix"
        args: ["develop", "--command", "true"]
        preload: true  // Runs in background
    }
]
```

### Source Hooks

Execute synchronously to capture environment variables. Source hooks ignore the preload flag.

```cue
hooks: onEnter: [
    {
        command: "devenv"
        args: ["shell", "dump"]
        source: true  // Captures environment variables
    }
]
```

## Preload Hooks (Background Execution)

Preload hooks solve the problem of slow environment preparation (e.g., Nix environments taking 30+ seconds) by running in the background.

### Benefits

- **No blocking**: Shell returns immediately after `cd`
- **Responsive shell**: Run `ls`, `cat`, etc. without waiting
- **Automatic waiting**: `cuenv exec` and `cuenv task` wait for completion
- **Cancellation**: Previous preload hooks cancel when changing directories

### Example

```cue
hooks: onEnter: [
    // Quick message - blocks briefly
    {
        command: "echo"
        args: ["Entering project..."]
    },

    // Slow preparation - runs in background
    {
        command: "nix"
        args: ["develop", "--command", "echo", "Ready"]
        preload: true
    },

    // Source environment - runs synchronously
    {
        command: "nix"
        args: ["develop", "--command", "sh", "-c", "export"]
        source: true
    }
]
```

## Hook Constraints

Hook constraints provide conditional execution based on the user's environment:

## Constraint Types

### CommandExists

Checks if a command is available in the system PATH using the `which` command.

```cue
constraints: [
    {
        commandExists: {
            command: "devenv"
        }
    },
    {
        commandExists: {
            command: "nix"
        }
    }
]
```

### ShellCommand

Runs an arbitrary shell command and checks if it succeeds (exit code 0). This is useful for complex environment validation that goes beyond simple command existence.

```cue
constraints: [
    {
        shellCommand: {
            command: "docker"
            args: ["info"]
        }
    },
    {
        shellCommand: {
            command: "nix"
            args: ["--version"]
        }
    }
]
```

## Usage

Constraints are defined as an array in the hook configuration. All constraints must pass for the hook to execute.

```cue
hooks: {
    onEnter: {
        command: "devenv"
        args: ["up"]
        constraints: [
            {
                commandExists: {
                    command: "devenv"
                }
            }
        ]
    }

    onExit: {
        command: "docker-compose"
        args: ["down"]
        constraints: [
            {
                commandExists: {
                    command: "docker-compose"
                }
            },
            {
                shellCommand: {
                    command: "docker"
                    args: ["info"]
                }
            }
        ]
    }
}
```

## Behavior

- **All constraints must pass**: If any constraint fails, the hook is skipped
- **Graceful failure**: Failed constraints don't cause errors, they simply skip hook execution
- **Logged results**: Constraint failures are logged at debug level for troubleshooting
- **User environment focus**: Constraints check the end user's environment for required tools
- **Async execution**: Constraint checking is non-blocking and handles errors gracefully

## Examples

### DevEnv Integration

Only run devenv setup if the tool is installed:

```cue
hooks: {
    onEnter: {
        command: "devenv"
        args: ["up"]
        constraints: [
            {
                commandExists: {
                    command: "devenv"
                }
            }
        ]
    }
}
```

### Docker Development Environment

Set up Docker containers only if Docker is available and running:

```cue
hooks: {
    onEnter: {
        command: "docker-compose"
        args: ["up", "-d"]
        constraints: [
            {
                commandExists: {
                    command: "docker"
                }
            },
            {
                commandExists: {
                    command: "docker-compose"
                }
            },
            {
                shellCommand: {
                    command: "docker"
                    args: ["info"]
                }
            }
        ]
    }
}
```

### Nix Environment Setup

Only run nix-specific setup if nix is available:

```cue
hooks: {
    onEnter: {
        command: "nix-shell"
        args: ["--run", "echo 'Nix environment ready'"]
        constraints: [
            {
                commandExists: {
                    command: "nix"
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

## Implementation Notes

- Constraint checking uses the same isolated environment as hook execution
- Command existence checking uses the `which` utility available on most Unix systems
- Shell commands run with the same environment variables available to hooks
- Constraint evaluation is fail-safe: errors during checking result in constraint failure rather than hook execution errors
- Focus is on validating external user environment rather than internal cuenv state
