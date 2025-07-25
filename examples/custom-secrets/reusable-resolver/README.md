# Reusable Custom Resolvers

This example demonstrates how to create reusable resolver definitions, similar to the built-in `#OnePasswordRef`.

## Configuration

```cue
// Define a reusable resolver
#VaultRef: cuenv.#Secret & {
    path:  string
    field: string
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=\(field)", path]
    }
}

// Use the resolver
DATABASE_PASSWORD: #VaultRef & {
    path:  "secret/myapp/database"
    field: "password"
}
```

## Usage

```bash
# Ensure vault is configured and authenticated
vault auth

# Run with custom resolver
cuenv run -- my-application
```

This shows how to create reusable resolver patterns that can be used multiple times with different parameters.