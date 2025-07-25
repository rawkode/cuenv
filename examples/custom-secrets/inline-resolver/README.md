# Custom Command Resolvers

This example demonstrates using an inline custom command resolver for secret management.

## Configuration

```cue
DATABASE_PASSWORD: {
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=password", "secret/myapp/database"]
    }
}
```

## Usage

```bash
# Ensure vault is configured and authenticated
vault auth

# Run with custom resolver
cuenv run -- my-application
```

This shows the basic pattern for custom resolvers that execute any command to fetch secrets.