---
title: Custom Secret Resolvers
description: Create custom secret resolvers to integrate with any secret management system
---

# Custom Secret Resolvers Examples

This guide demonstrates how to create custom secret resolvers using cuenv's flexible resolver framework.

## Examples

- **Inline Resolver** - Basic inline custom resolver example
- **Reusable Resolver** - Reusable resolver definition pattern

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

## Common Integrations

### AWS Secrets Manager

```cue
#AWSSecret: cuenv.#Secret & {
    secretId: string
    region:   string | *"us-east-1"
    resolver: {
        command: "aws"
        args: ["secretsmanager", "get-secret-value",
               "--secret-id", secretId,
               "--region", region,
               "--query", "SecretString",
               "--output", "text"]
    }
}

// Usage
DATABASE_PASSWORD: #AWSSecret & {
    secretId: "prod/database/password"
    region: "us-west-2"
}
```

### HashiCorp Vault

```cue
#VaultSecret: cuenv.#Secret & {
    path:  string
    field: string
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=\(field)", path]
    }
}

// Usage
API_KEY: #VaultSecret & {
    path: "secret/api-keys"
    field: "production"
}
```

### Azure Key Vault

```cue
#AzureSecret: cuenv.#Secret & {
    vaultName: string
    secretName: string
    resolver: {
        command: "az"
        args: ["keyvault", "secret", "show",
               "--vault-name", vaultName,
               "--name", secretName,
               "--query", "value",
               "--output", "tsv"]
    }
}

// Usage
DB_CONNECTION: #AzureSecret & {
    vaultName: "my-keyvault"
    secretName: "database-connection-string"
}
```

### Google Secret Manager

```cue
#GCPSecret: cuenv.#Secret & {
    project:  string
    secret:   string
    version:  string | *"latest"
    resolver: {
        command: "gcloud"
        args: ["secrets", "versions", "access", version,
               "--secret", secret,
               "--project", project,
               "--quiet"]
    }
}

// Usage
OAUTH_SECRET: #GCPSecret & {
    project: "my-gcp-project"
    secret: "oauth-client-secret"
}
```

## Usage

```bash
cd examples/custom-secrets/inline-resolver
cuenv run -- your-application
```

## Best Practices

1. **Error Handling**: Ensure your secret commands handle errors gracefully
2. **Caching**: Consider caching secrets to avoid repeated API calls
3. **Security**: Never log secret values in resolver commands
4. **Testing**: Test resolvers with mock commands during development

For more information, see the [Secrets Management Guide](/guides/secrets/).
