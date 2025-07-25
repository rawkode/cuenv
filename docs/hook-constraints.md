# Hook Constraints

This document describes the hook constraints feature in cuenv, which allows hooks to run conditionally based on system state.

## Overview

Hook constraints provide a way to ensure hooks only execute when specific conditions are met. This is particularly useful for:

- Running setup hooks only when required tools are installed
- Conditional cleanup based on environment state  
- Avoiding errors when dependencies are missing
- Creating portable environments that gracefully handle missing components

## Constraint Types

### CommandExists

Checks if a command is available in the system PATH using the `which` command.

```cue
constraints: [
    {
        commandExists: {
            command: "devenv"
        }
    }
]
```

### FileExists  

Checks if a file or directory exists. Supports shell variable expansion.

```cue
constraints: [
    {
        fileExists: {
            path: "devenv.nix"
        }
    },
    {
        fileExists: {
            path: "$HOME/.config/myapp/config.yaml"
        }
    }
]
```

### EnvVarSet

Checks if an environment variable is set (non-empty).

```cue
constraints: [
    {
        envVarSet: {
            var: "DEVENV_ROOT"
        }
    }
]
```

### EnvVarEquals

Checks if an environment variable equals a specific value.

```cue
constraints: [
    {
        envVarEquals: {
            var: "CLEANUP_MODE"
            value: "auto"
        }
    }
]
```

### ShellCommand

Runs an arbitrary shell command and checks if it succeeds (exit code 0).

```cue
constraints: [
    {
        shellCommand: {
            command: "test"
            args: ["-f", "/tmp/required_file"]
        }
    },
    {
        shellCommand: {
            command: "docker"
            args: ["info"]
        }
    }
]
```

## Usage

Constraints are defined as an array in the hook configuration. All constraints must pass for the hook to execute.

```cue
hooks: {
    onEnter: {
        command: "setup-dev-env"
        args: ["--quick"]
        constraints: [
            {
                commandExists: {
                    command: "docker"
                }
            },
            {
                fileExists: {
                    path: "docker-compose.yml"
                }
            },
            {
                envVarSet: {
                    var: "DOCKER_HOST"
                }
            }
        ]
    }
    
    onExit: {
        command: "cleanup"
        args: []
        constraints: [
            {
                envVarEquals: {
                    var: "AUTO_CLEANUP"
                    value: "true"
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
- **Environment access**: Constraints can access both cuenv-managed and system environment variables
- **Async execution**: Constraint checking is non-blocking and handles errors gracefully

## Examples

### DevEnv Integration

Only run devenv setup if the tool is installed and configuration exists:

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
            },
            {
                fileExists: {
                    path: "devenv.nix"
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
                shellCommand: {
                    command: "docker"
                    args: ["info"]
                }
            },
            {
                fileExists: {
                    path: "docker-compose.yml"
                }
            }
        ]
    }
}
```

### Conditional Cleanup

Only run cleanup in specific environments:

```cue
hooks: {
    onExit: {
        command: "cleanup-temp-files"
        args: []
        constraints: [
            {
                envVarEquals: {
                    var: "NODE_ENV"
                    value: "development"
                }
            },
            {
                envVarSet: {
                    var: "TEMP_DIR"
                }
            }
        ]
    }
}
```

## Implementation Notes

- Constraint checking uses the same isolated environment as hook execution
- File path expansion supports standard shell variables (`$HOME`, `$PWD`, etc.)
- Command existence checking uses the `which` utility available on most Unix systems
- Shell commands run with the same environment variables available to hooks
- Constraint evaluation is fail-safe: errors during checking result in constraint failure rather than hook execution errors