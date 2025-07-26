# Inferred Security Example

This example demonstrates cuenv's ability to automatically infer security restrictions based on task inputs and outputs.

## Features

- **Automatic path inference**: Security restrictions are derived from declared inputs/outputs
- **Audit mode**: Use `--audit` flag to see what access a task actually needs
- **Manual overrides**: Can combine inferred restrictions with explicit ones

## Usage

```bash
cd examples/inferred-security
cuenv task process-data    # Automatically restricts access to input/output paths
cuenv task build-project   # Combines inferred paths with network restrictions
cuenv task --audit audit-example  # Run in audit mode to see access patterns
```

## Inferred Security

When `inferFromInputsOutputs: true` is set, cuenv automatically:

1. Grants read-only access to paths specified in `inputs`
2. Grants read-write access to paths specified in `outputs`
3. Blocks access to all other filesystem paths

```cue
tasks: {
    "process-data": {
        inputs: ["./input.txt", "./config.json"]
        outputs: ["./output.txt", "./logs/"]
        security: {
            inferFromInputsOutputs: true
            // Additional manual restrictions can be added
            readOnlyPaths: ["/usr/bin", "/bin"]
        }
    }
}
```

## Audit Mode

Use the `--audit` flag to run tasks without restrictions but log all access:

```bash
cuenv task --audit audit-example
```

This helps you understand what access your tasks actually need before applying restrictions.
