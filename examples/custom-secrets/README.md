# Custom Secret Resolvers Examples

This directory demonstrates how to create custom secret resolvers using cuenv's flexible resolver framework.

## Examples

- **`inline-resolver/`** - Basic inline custom resolver example
- **`reusable-resolver/`** - Reusable resolver definition pattern

## How Custom Resolvers Work

Custom resolvers allow you to integrate with any secret management system by defining:

1. **Command**: The executable to run
2. **Arguments**: Parameters to pass to the command

## Basic Pattern

```cue
MY_SECRET: {
    resolver: {
        command: "your-secret-command"
        args: ["--get", "secret-name"]
    }
}
```

## Reusable Pattern

```cue
#VaultRef: cuenv.#Secret & {
    path:  string
    field: string
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=\(field)", path]
    }
}
```

## Usage

```bash
cd examples/custom-secrets/inline-resolver
cuenv run -- your-application
```
