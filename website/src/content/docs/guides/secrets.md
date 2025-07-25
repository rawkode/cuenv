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

cuenv supports several secret management systems out of the box, plus custom command-based resolvers for any system.

### Built-in Resolvers

#### 1Password

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

```cue title="env.cue"
package env

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

Beyond the built-in resolvers, cuenv supports custom command-based secret resolvers that can integrate with any secret management system. This powerful feature allows you to:

- Integrate with enterprise secret management systems
- Use custom authentication mechanisms  
- Support legacy secret storage systems
- Create specialized secret transformation logic

#### How Custom Resolvers Work

Custom resolvers use the `#Resolver` schema to define a command and arguments that cuenv will execute to retrieve secrets:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    MY_SECRET: {
        resolver: {
            command: "your-secret-command"
            args: ["--get", "secret-name"]
        }
    }
}
```

When you run `cuenv run`, it will:
1. Execute the specified command with the given arguments
2. Capture the stdout as the secret value
3. Automatically obfuscate the secret in logs and output
4. Make the secret available to your application

#### HashiCorp Vault Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Key-Value secrets
    DATABASE_PASSWORD: {
        resolver: {
            command: "vault"
            args: [
                "kv", "get", "-field=password",
                "secret/myapp/database"
            ]
        }
    }

    // Dynamic AWS credentials from Vault
    AWS_ACCESS_KEY_ID: {
        resolver: {
            command: "vault"
            args: [
                "read", "-field=access_key",
                "aws/creds/my-role"
            ]
        }
    }

    // Environment-specific secrets
    environment: {
        production: {
            DATABASE_PASSWORD: {
                resolver: {
                    command: "vault"
                    args: [
                        "kv", "get", "-field=password",
                        "secret/prod/database"
                    ]
                }
            }
        }
    }
}
```

#### AWS Secrets Manager Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_PASSWORD: {
        resolver: {
            command: "aws"
            args: [
                "secretsmanager", "get-secret-value",
                "--secret-id", "myapp/database/password", 
                "--query", "SecretString",
                "--output", "text"
            ]
        }
    }

    // Extract from JSON secret
    OAUTH_CLIENT_SECRET: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                "aws secretsmanager get-secret-value --secret-id myapp/oauth --query SecretString --output text | jq -r .client_secret"
            ]
        }
    }
}
```

#### Azure Key Vault Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_PASSWORD: {
        resolver: {
            command: "az"
            args: [
                "keyvault", "secret", "show",
                "--vault-name", "myapp-keyvault",
                "--name", "database-password",
                "--query", "value",
                "--output", "tsv"
            ]
        }
    }
}
```

#### SOPS (Mozilla Secrets OPerationS) Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_PASSWORD: {
        resolver: {
            command: "sops"
            args: [
                "--decrypt", "--extract", '["database"]["password"]',
                "secrets.yaml"
            ]
        }
    }

    // Extract from JSON with jq
    API_KEY: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                "sops --decrypt secrets.json | jq -r .api_key"
            ]
        }
    }
}
```

#### Unix Pass Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_PASSWORD: {
        resolver: {
            command: "pass"
            args: ["myapp/database/password"]
        }
    }

    // Get specific line from pass entry
    SMTP_USERNAME: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                "pass email/smtp | head -n 1"
            ]
        }
    }
}
```

#### Bitwarden CLI Example

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    DATABASE_PASSWORD: {
        resolver: {
            command: "bw"
            args: [
                "get", "password",
                "MyApp Database"
            ]
        }
    }

    // Extract custom field with jq
    OAUTH_CLIENT_SECRET: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                "bw get item 'OAuth Client' | jq -r .fields[0].value"
            ]
        }
    }
}
```

#### Advanced Custom Transformations

Custom resolvers can perform complex transformations and validations:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Base64 decode a secret
    DECODED_SECRET: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                "echo 'aGVsbG8gd29ybGQ=' | base64 -d"
            ]
        }
    }

    // Validate secret format
    VALIDATED_API_KEY: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                '''
                key=$(vault kv get -field=api_key secret/myapp)
                if [[ ${#key} -lt 32 ]]; then
                    echo "Error: API key too short" >&2
                    exit 1
                fi
                echo "$key"
                '''
            ]
        }
    }

    // Composite secret from multiple sources
    DATABASE_URL: {
        resolver: {
            command: "sh"
            args: [
                "-c",
                '''
                user=$(vault kv get -field=username secret/db)
                pass=$(vault kv get -field=password secret/db)
                host=$(consul kv get database/host)
                echo "postgres://$user:$pass@$host:5432/myapp"
                '''
            ]
        }
    }
}
```

#### Security Best Practices for Custom Resolvers

1. **Command Security**: Only use trusted commands and validate inputs
2. **Error Handling**: Commands should exit with non-zero status on failure
3. **Output Formatting**: Commands should output only the secret value (no extra text)
4. **Authentication**: Ensure CLI tools are properly authenticated before running cuenv
5. **Least Privilege**: Grant minimal permissions to secret access commands
6. **Command Injection**: Be careful with shell commands - prefer direct CLI calls when possible

#### Troubleshooting Custom Resolvers

**Command not found:**
```bash
# Ensure the command is in PATH
which vault
which aws
which az
```

**Authentication errors:**
```bash
# Verify authentication for each tool
vault auth -method=userpass username=myuser
aws sts get-caller-identity
az account show
```

**Secret format errors:**
```bash
# Test commands manually first
vault kv get -field=password secret/myapp/database
aws secretsmanager get-secret-value --secret-id myapp/db --query SecretString --output text
```

**Permission denied:**
```bash
# Check permissions
vault auth list
aws iam get-user
az role assignment list --assignee $(az account show --query user.name --output tsv)
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

import "github.com/rawkode/cuenv"

DATABASE_URL: {
    resolver: {
        command: "vault"
        args: ["kv", "get", "-field=database_url", "secret/myapp"]
    }
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

import "github.com/rawkode/cuenv"

DATABASE_PASSWORD: cuenv.#OnePasswordRef & {
    ref: "op://Personal/Database/password"
}
API_KEY: "gcp-secret://my-project/api-key"
```

The key difference is that with cuenv, secrets are declarative and only resolved when using `cuenv run`.

## Complete Examples

For complete, working examples of custom secret resolvers, see:

- **[examples/custom-secrets/](https://github.com/rawkode/cuenv/tree/main/examples/custom-secrets)** - Comprehensive examples for various secret management systems
- **HashiCorp Vault** - Enterprise secret management
- **AWS Secrets Manager** - Cloud-native AWS secrets
- **Azure Key Vault** - Microsoft Azure secret management
- **SOPS** - File-based encryption with git workflows
- **pass** - Unix password manager integration
- **Bitwarden** - Popular password manager CLI
- **Custom transformations** - Advanced patterns and validations

Each example includes both CUE configuration files and detailed setup instructions.
