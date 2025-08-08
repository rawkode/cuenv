---
title: Security Configuration Reference
description: Complete reference for cuenv's security configuration options
---

This page provides a complete reference for all security configuration options available in cuenv tasks.

## Security Configuration Schema

The `security` field in task definitions accepts the following options:

```cue
tasks: {
    "task-name": {
        security: {
            // Filesystem restrictions
            restrictDisk: bool
            readOnlyPaths: [...string]
            readWritePaths: [...string]
            denyPaths: [...string]

            // Network restrictions
            restrictNetwork: bool
            allowedHosts: [...string]

            // Automatic inference
            inferFromInputsOutputs: bool
        }
    }
}
```

## Filesystem Options

### `restrictDisk`

- **Type**: `bool`
- **Default**: `false`
- **Description**: Enable Landlock-based filesystem sandboxing

When enabled, the task can only access paths explicitly allowed through `readOnlyPaths` and `readWritePaths`.

### `readOnlyPaths`

- **Type**: `[...string]`
- **Default**: `[]`
- **Description**: Paths the task can read from but not write to

Example:

```cue
readOnlyPaths: [
    "/usr",           // System binaries
    "/lib",           // System libraries
    "./src",          // Source code
    "./config.json"   // Specific file
]
```

### `readWritePaths`

- **Type**: `[...string]`
- **Default**: `[]`
- **Description**: Paths the task can both read from and write to

Example:

```cue
readWritePaths: [
    "/tmp",          // Temporary files
    "./build",       // Build output
    "./cache",       // Cache directory
    "./output.log"   // Specific output file
]
```

### `denyPaths`

- **Type**: `[...string]`
- **Default**: `[]`
- **Description**: Paths to explicitly deny access to (overrides allow rules)

Use this to block access to sensitive files within allowed directories:

```cue
readOnlyPaths: ["/etc"]
denyPaths: ["/etc/shadow", "/etc/sudoers"]
```

## Network Options

### `restrictNetwork`

- **Type**: `bool`
- **Default**: `false`
- **Description**: Enable Landlock-based network sandboxing (requires Linux 6.7+)

When enabled, the task can only connect to hosts listed in `allowedHosts`.

### `allowedHosts`

- **Type**: `[...string]`
- **Default**: `[]`
- **Description**: Hostnames the task is allowed to connect to

Example:

```cue
allowedHosts: [
    "api.github.com",
    "registry.npmjs.org",
    "localhost",
    "192.168.1.100"
]
```

**Note**: Hostnames are resolved to IP addresses when restrictions are applied. Both IPv4 and IPv6 addresses are supported.

## Inference Options

### `inferFromInputsOutputs`

- **Type**: `bool`
- **Default**: `false`
- **Description**: Automatically infer filesystem restrictions from task inputs/outputs

When enabled:

- Paths listed in task `inputs` get read-only access
- Paths listed in task `outputs` get read-write access
- Parent directories get appropriate access
- System paths for executables are automatically included

Example:

```cue
tasks: {
    "process": {
        inputs: ["./data/", "./scripts/process.py"]
        outputs: ["./results/"]
        security: {
            inferFromInputsOutputs: true
            // Automatically grants:
            // - Read: ./data/, ./scripts/process.py
            // - Write: ./results/
            // - Read: /usr, /lib, /bin (for Python)
        }
    }
}
```

## Path Resolution

### Relative Paths

Relative paths are resolved from the directory containing the `env.cue` file:

```cue
readOnlyPaths: [
    "./src",        // Resolves to $CUE_DIR/src
    "../shared"     // Resolves to $CUE_DIR/../shared
]
```

### Glob Patterns

Glob patterns are **not** supported in security paths. Each path must be explicitly listed:

```cue
// This will NOT work:
readOnlyPaths: ["./src/**/*.js"]

// Do this instead:
readOnlyPaths: ["./src"]  // Grants access to entire directory
```

### Symbolic Links

Symbolic links require access to both the link and the target:

```cue
readOnlyPaths: [
    "/usr/bin/python",     // The symlink
    "/usr/bin/python3.11"  // The actual binary
]
```

## Command Line Options

### Audit Mode

Run any task with `--audit` to log access without enforcing restrictions:

```bash
cuenv task --audit my-task
```

Output includes:

- Files opened for reading
- Files opened for writing
- Network connections attempted
- Suggested security configuration

### Force Enable/Disable

Override task security settings from the command line:

```bash
# Force enable all security restrictions
cuenv task --force-sandbox my-task

# Disable all security restrictions
cuenv task --no-sandbox my-task
```

## Environment Variables

### `CUENV_SECURITY_AUDIT`

Enable audit mode for all tasks:

```bash
export CUENV_SECURITY_AUDIT=1
cuenv task my-task
```

### `CUENV_SECURITY_DISABLE`

Disable all security restrictions:

```bash
export CUENV_SECURITY_DISABLE=1
cuenv task my-task
```

## Error Messages

### Common Errors

1. **"Landlock not available"**
   - Kernel doesn't support Landlock
   - Security restrictions will be ignored

2. **"Permission denied"**
   - Path not in allowed lists
   - Run with `--audit` to debug

3. **"Network connection refused"**
   - Host not in `allowedHosts`
   - Check hostname resolution

## Examples

### Minimal Build Security

```cue
tasks: {
    build: {
        command: "go build -o app"
        security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "."]
            readWritePaths: ["/tmp", "./app"]
        }
    }
}
```

### Network API Client

```cue
tasks: {
    "fetch-data": {
        command: "curl https://api.example.com/data"
        security: {
            restrictNetwork: true
            allowedHosts: ["api.example.com"]
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/etc/ssl"]
            readWritePaths: ["./data.json"]
        }
    }
}
```

### Complex Build Pipeline

```cue
tasks: {
    "full-build": {
        command: "make all"
        inputs: ["./src", "./Makefile", "./vendor"]
        outputs: ["./dist", "./build"]
        security: {
            inferFromInputsOutputs: true
            restrictNetwork: true
            allowedHosts: [
                "github.com",
                "proxy.golang.org",
                "sum.golang.org"
            ]
            // Additional paths for build tools
            readOnlyPaths: [
                "/usr", "/lib", "/bin",
                "~/.cache/go-build"
            ]
            readWritePaths: ["/tmp"]
        }
    }
}
```
