---
title: Secret Management
description: Secrets that don't end up in your git history
---

Stop commenting "# TODO: Don't commit this" above your API keys. cuenv has actual secret management that works.

## How It Works

Write `op://vault/item/field` or `gcp-secret://project/name`. Run `cuenv run`. Secrets resolve. No plaintext files. No git accidents.

The important bits:

- Secrets only resolve with `cuenv run` (not in your regular shell)
- Values are hidden in logs/output (shows `***` instead)
- Zero setup beyond having `op` or `gcloud` installed

## Supported Secret Managers

### 1Password

1Password is a popular password manager that provides a CLI for programmatic access.

#### Setup

1. Install the [1Password CLI](https://developer.1password.com/docs/cli/):

   ```bash
   # macOS
   brew install --cask 1password-cli

   # Linux (example for Ubuntu/Debian)
   curl -sS https://downloads.1password.com/linux/keys/1password.asc | \
     sudo gpg --dearmor --output /usr/share/keyrings/1password-archive-keyring.gpg
   ```

1. Sign in to your 1Password account:

   ```bash
   op signin
   ```

#### Secret Reference Format

1Password secrets use the `op://` URL scheme:

```
op://vault/item/field
op://vault/item/section/field
```

#### Examples

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// Basic password reference
DATABASE_PASSWORD: cuenv.#OnePasswordRef & {
    ref: "op://Personal/PostgreSQL/password"
}

// API key from a specific vault
STRIPE_API_KEY: cuenv.#OnePasswordRef & {
    ref: "op://Work/Stripe API/secret_key"
}

// Reference with section
AWS_SECRET_KEY: cuenv.#OnePasswordRef & {
    ref: "op://DevOps/AWS/credentials/secret_key"
}

// Using in connection strings
DB_USER: "myapp"
DB_PASS: cuenv.#OnePasswordRef & {
    ref: "op://Personal/MyApp DB/password"
}
DB_HOST: "db.example.com"
DATABASE_URL: "postgres://\(DB_USER):\(DB_PASS)@\(DB_HOST):5432/myapp"
```

### Google Cloud Platform (GCP) Secrets Manager

GCP Secrets Manager provides a secure and convenient way to store API keys, passwords, certificates, and other sensitive data.

#### Setup

1. Install the [Google Cloud SDK](https://cloud.google.com/sdk/docs/install):

   ```bash
   # Download and install gcloud CLI
   curl https://sdk.cloud.google.com | bash
   ```

1. Authenticate with your Google account:

   ```bash
   gcloud auth login
   ```

1. Set your default project:

   ```bash
   gcloud config set project YOUR_PROJECT_ID
   ```

#### Secret Reference Format

GCP secrets use the `gcp-secret://` URL scheme:

```
gcp-secret://project-id/secret-name
gcp-secret://project-id/secret-name/version
```

#### Examples

```cue title="env.cue"
package env

// Latest version of a secret
API_KEY: "gcp-secret://my-project/api-key"

// Specific version
DATABASE_PASSWORD: "gcp-secret://my-project/db-password/2"

// Using project variables
_gcpProject: "prod-project-123"
SMTP_PASSWORD: "gcp-secret://\(_gcpProject)/smtp-credentials"

// Combined with other values
SERVICE_ACCOUNT_KEY: "gcp-secret://my-project/service-account-key"
```

## Structured Secret Definitions

For better type safety and documentation, you can use structured format for secrets:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// Use cuenv's built-in structured format
DATABASE_PASSWORD: cuenv.#OnePasswordRef & {
    ref: "op://Personal/Database/password"
}

// Define custom type for GCP secrets (not built-in yet)
#GcpSecret: {
    project: string
    secret: string
    version?: string | *"latest"
}

API_SECRET: #GcpSecret & {
    project: "my-project"
    secret: "api-secret-key"
    version: "latest"
}

SIGNING_KEY: #GcpSecret & {
    project: "prod-project"
    secret: "jwt-signing-key"
    version: "3"  // Pin to specific version
}
```

### Custom Command Secret Resolvers

For integrating with other secret management systems, you can create custom command-based resolvers.

#### Inline Resolvers

Define custom resolvers directly on individual secrets:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Custom resolver for HashiCorp Vault
    DATABASE_PASSWORD: cuenv.#Secret & {
        resolver: {
            command: "vault"
            args: ["kv", "get", "-field=password", "secret/myapp/database"]
        }
    }
}
```

#### Reusable Resolver Definitions

For better reusability, create custom resolver types:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

// Define a reusable resolver
#VaultRef: cuenv.#Secret & {
    path:  string
    field: string
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=\(field)", path]
    }
}

env: cuenv.#Env & {
    // Use the reusable resolver
    DATABASE_PASSWORD: #VaultRef & {
        path:  "secret/myapp/database"
        field: "password"
    }
}
```

## Using Secrets with cuenv run

Secrets are only resolved when using the `cuenv run` command:

```bash
# Create env.cue with secrets
cat > env.cue << 'EOF'
package env

import "github.com/rawkode/cuenv/cue"

DATABASE_URL: "postgres://user:pass@localhost/db"
API_KEY: cuenv.#OnePasswordRef & {
    ref: "op://Work/MyApp/api_key"
}
JWT_SECRET: "gcp-secret://my-project/jwt-secret"
EOF

# Run a command with resolved secrets
cuenv run node server.js

# Secrets are resolved and passed to the command
# The actual secret values are available in the process environment

# Regular shell usage doesn't resolve secrets
echo $API_KEY
# Output: op://Work/MyApp/api_key (not resolved)
```

## Secret Obfuscation

cuenv automatically obfuscates resolved secret values in command output:

```bash
# If API_KEY resolves to "sk_live_abcd1234"
cuenv run sh -c 'echo "API Key: $API_KEY"'
# Output: API Key: ***********

# Obfuscation works in stderr too
cuenv run sh -c 'echo "Error: Invalid key $API_KEY" >&2'
# Stderr: Error: Invalid key ***********

# Multiple secrets are each obfuscated
cuenv run sh -c 'echo "$DATABASE_PASSWORD $API_KEY"'
# Output: *********** ***********
```

## Security Best Practices

### 1. Never Commit Secret Values

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// DON'T: Never put actual secrets in env.cue
API_KEY: "sk_live_abcd1234"  // Bad!

// DO: Always use secret references
API_KEY: cuenv.#OnePasswordRef & {
    ref: "op://Work/Stripe/secret_key"
}  // Good!
```

### 2. Use Specific Vaults/Projects

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// Be specific about vault/project names
PROD_DB_PASS: cuenv.#OnePasswordRef & {
    ref: "op://Production/Database/password"
}
DEV_DB_PASS: cuenv.#OnePasswordRef & {
    ref: "op://Development/Database/password"
}

// Use different GCP projects for different environments
PROD_API_KEY: "gcp-secret://prod-project/api-key"
DEV_API_KEY: "gcp-secret://dev-project/api-key"
```

### 3. Principle of Least Privilege

Only grant access to secrets that are needed:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// Use capability tags to limit secret exposure
AWS_ACCESS_KEY: cuenv.#OnePasswordRef & {
    ref: "op://AWS/prod-access/key"
} @capability("aws")
AWS_SECRET_KEY: cuenv.#OnePasswordRef & {
    ref: "op://AWS/prod-access/secret"
} @capability("aws")

// These will only be available when running AWS commands
```

### 4. Rotate Secrets Regularly

When using GCP Secrets Manager with versions:

```cue title="env.cue"
package env

// Pin to specific versions during rotation
API_KEY: "gcp-secret://my-project/api-key/5"  // Current version

// After rotation, update to new version
// API_KEY: "gcp-secret://my-project/api-key/6"  // New version
```

### 5. Audit Secret Access

Both 1Password and GCP provide audit logs:

- **1Password**: Check activity log in 1Password app
- **GCP**: Use Cloud Audit Logs to track secret access

## Environment-Specific Secrets

Use different secrets for different environments:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

// Base configuration
APP_NAME: "myapp"

// Environment-specific secrets
environment: {
    development: {
        DATABASE_URL: "postgres://localhost/myapp_dev"
        API_KEY: cuenv.#OnePasswordRef & {
            ref: "op://Development/MyApp/api_key"
        }
    }
    production: {
        DATABASE_URL: cuenv.#OnePasswordRef & {
            ref: "op://Production/MyApp/database_url"
        }
        API_KEY: cuenv.#OnePasswordRef & {
            ref: "op://Production/MyApp/api_key"
        }
    }
}
```

Usage:

```bash
# Development environment
cuenv run -e development -- npm start

# Production environment
cuenv run -e production -- npm start
```

## Troubleshooting

### 1Password Issues

**Not signed in:**

```bash
# Error: You are not currently signed in
op signin
```

**Item not found:**

```bash
# Check exact vault and item names
op item list --vault="Work"
op item get "MyApp API" --vault="Work"
```

**Field not found:**

```bash
# List all fields in an item
op item get "MyApp API" --format json | jq '.fields[].label'
```

### GCP Issues

**Not authenticated:**

```bash
# Re-authenticate
gcloud auth login
```

**Secret not found:**

```bash
# List secrets in project
gcloud secrets list --project=my-project

# Check secret versions
gcloud secrets versions list my-secret --project=my-project
```

**Permission denied:**

```bash
# Grant secret accessor role
gcloud secrets add-iam-policy-binding my-secret \
    --member="user:you@example.com" \
    --role="roles/secretmanager.secretAccessor" \
    --project=my-project
```

## Migration Guide

### From .env Files

Before (`.env`):

```bash
DATABASE_URL=postgres://user:pass@localhost/db
API_KEY=sk_live_abcd1234
JWT_SECRET=super-secret-key
```

After (`env.cue`):

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

DATABASE_URL: cuenv.#OnePasswordRef & {
    ref: "op://Personal/MyApp/database_url"
}
API_KEY: cuenv.#OnePasswordRef & {
    ref: "op://Work/Stripe/secret_key"
}
JWT_SECRET: "gcp-secret://my-project/jwt-secret"
```

### From direnv

Before (`.envrc`):

```bash
export DATABASE_PASSWORD=$(op read "op://Personal/Database/password")
export API_KEY=$(gcloud secrets versions access latest --secret=api-key)
```

After (`env.cue`):

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv/cue"

DATABASE_PASSWORD: cuenv.#OnePasswordRef & {
    ref: "op://Personal/Database/password"
}
API_KEY: "gcp-secret://my-project/api-key"
```

The key difference is that with cuenv, secrets are declarative and only resolved when using `cuenv run`.
