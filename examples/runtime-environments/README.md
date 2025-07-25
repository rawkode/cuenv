# Runtime Environments Example

This example demonstrates cuenv's runtime environment support, which allows you to execute tasks in different isolated environments.

## Supported Runtime Environments

### Host Runtime (Default)
Tasks run directly on the host system using the current shell environment.

```cue
tasks: {
    "my-task": {
        description: "Task running on host"
        command: "echo 'Hello from host'"
        // No runtime specified - defaults to host
    }
}
```

### Nix Runtime
Execute tasks within Nix environments for reproducible builds.

```cue
tasks: {
    "nix-shell-task": {
        description: "Task with nix-shell packages"
        command: "node --version"
        runtime: {
            type: "nix"
            config: {
                shell: "nodejs npm"  // Packages to include
                pure: false          // Allow host environment
            }
        }
    }
    
    "nix-flake-task": {
        description: "Task using nix flake"
        command: "echo 'In flake environment'"
        runtime: {
            type: "nix"
            config: {
                flake: "."     // Use current directory flake
                pure: false
            }
        }
    }
}
```

### Docker Runtime
Execute tasks in Docker containers for maximum isolation.

```cue
tasks: {
    "docker-task": {
        description: "Task in Docker container"
        command: "python --version"
        runtime: {
            type: "docker"
            config: {
                image: "python:3.11"
                workDir: "/workspace"
                env: {
                    PYTHONPATH: "/workspace"
                }
                volumes: [
                    "/host/data:/container/data:ro"
                ]
                rm: true  // Remove container after execution
            }
        }
    }
}
```

### Podman Runtime
Execute tasks in Podman containers (rootless alternative to Docker).

```cue
tasks: {
    "podman-task": {
        description: "Task in Podman container"
        command: "go version"
        runtime: {
            type: "podman"
            config: {
                image: "golang:1.21"
                workDir: "/workspace"
                network: "host"
                rm: true
            }
        }
    }
}
```

### BuildKit Runtime
Execute tasks using Docker BuildKit for advanced build scenarios.

```cue
tasks: {
    "buildkit-task": {
        description: "Task using BuildKit"
        command: "echo 'Built with BuildKit'"
        runtime: {
            type: "buildkit"
            config: {
                image: "alpine:latest"
                context: "."
                buildArgs: {
                    NODE_ENV: "production"
                }
            }
        }
    }
}
```

## Running the Examples

### List Available Tasks
```bash
cuenv run
```

### Run Host Task
```bash
cuenv run host-task
```

### Run Docker Task (requires Docker)
```bash
cuenv run docker-task
```

### Run Nix Task (requires Nix)
```bash
cuenv run nix-task
```

### Run Complex Docker Task
```bash
cuenv run docker-complex
```

## Runtime Requirements

- **Host**: Always available
- **Nix**: Requires `nix` command to be available
- **Docker**: Requires `docker` command and Docker daemon
- **Podman**: Requires `podman` command
- **BuildKit**: Requires `docker` with BuildKit support (Docker 18.06+)

## Runtime Features

### Environment Variable Inheritance
All runtime environments automatically inherit environment variables defined in the `env` section of your CUE file.

### Working Directory Mounting
Container runtimes (Docker, Podman) automatically mount the current working directory into the container for seamless file access.

### Error Handling
If a runtime is not available on the system, cuenv will report an error with instructions on what's missing.

### Security
Container runtimes provide isolation from the host system while still allowing controlled access to necessary files and environment variables.

## Best Practices

1. **Use specific image tags** for reproducible builds
2. **Set `rm: true`** for containers to avoid accumulating stopped containers
3. **Use read-only volumes** when possible for security
4. **Prefer Nix for reproducible development environments**
5. **Use containers for isolated testing and production builds**