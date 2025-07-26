# Secure Task Example

This example demonstrates how to use cuenv's security features to restrict filesystem and network access for tasks.

## Features

- **Filesystem restrictions**: Control which paths tasks can read from or write to
- **Network restrictions**: Limit which hosts tasks can connect to
- **Landlock-based sandboxing**: Uses Linux kernel security features (requires kernel 5.13+)

## Usage

```bash
cd examples/secure-task
cuenv task secure-build    # Build with filesystem restrictions
cuenv task network-task    # Task with network restrictions
cuenv task fully-restricted # Task with both disk and network restrictions
```

## Security Configuration

Tasks can specify security restrictions using the `security` field:

```cue
tasks: {
    "secure-build": {
        security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp", "./build"]
        }
    }
}
```

See `env.cue` for complete examples.
