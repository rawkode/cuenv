---
title: Hook Constraints
description: Hook constraints for conditional execution based on user environment
---

# Hook Constraints

Hook constraints provide a way to ensure hooks only execute when required tools are available in the end user's environment. This is particularly useful for:

- Running setup hooks only when required tools are installed (e.g., devenv, nix, flox)
- Avoiding errors when dependencies are missing from the user's system
- Creating portable environments that gracefully handle missing tools
- Checking complex environment conditions with custom shell commands

Since cuenv runs tasks in an isolated environment where files and environment variables are already defined, constraints focus on validating the external user environment rather than checking internal state.

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
