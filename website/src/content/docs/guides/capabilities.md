---
title: Capabilities
description: AWS creds for AWS CLI only. Not for that sketchy npm script.
---

Your AWS credentials shouldn't be available to every random script. Capabilities fix that.

## The Problem

Every tool gets every env var. Your build script has your production database password. That random npm package can read your AWS keys. This is insane.

## The Solution

Tag sensitive vars with capabilities. They only load for the right commands.

## Basic Usage

### Tagging Variables

Use the `@capability()` attribute to tag variables:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // General variables (always available)
    APP_NAME: "myapp"
    LOG_LEVEL: "info"

    // AWS credentials (only with 'aws' capability)
    AWS_ACCESS_KEY: "AKIA..." @capability("aws")
    AWS_SECRET_KEY: "secret..." @capability("aws")
    AWS_REGION: "us-east-1" @capability("aws")

    // Database credentials (only with 'database' capability)
    DATABASE_URL: "postgres://user:pass@host/db" @capability("database")
    DATABASE_POOL_SIZE: 10 @capability("database")

    // GitHub token (only with 'github' capability)
    GITHUB_TOKEN: "ghp_..." @capability("github")
    GITHUB_ORG: "myorg" @capability("github")
}
```

### Using Capabilities

There are three ways to enable capabilities:

1. **Command-line flag:**

   ```bash
   cuenv run -c aws,database -- terraform apply
   ```

1. **Environment variable:**

   ```bash
   CUENV_CAPABILITIES=aws,database cuenv run -- terraform apply
   ```

1. **Automatic inference** (based on command mapping):

   ```bash
   cuenv run -- aws s3 ls  # Automatically gets 'aws' capability
   ```

## Command Mapping

Define which commands automatically receive which capabilities:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Variables with capabilities
    AWS_ACCESS_KEY: "key" @capability("aws")
    CLOUDFLARE_API_TOKEN: "token" @capability("cloudflare")
    DATABASE_URL: "postgres://..." @capability("database")

    // Map commands to their required capabilities
    Commands: {
        // Terraform needs multiple providers
        terraform: {
            capabilities: ["aws", "cloudflare", "database"]
        }

        // AWS CLI needs AWS credentials
        aws: {
            capabilities: ["aws"]
        }

        // Database tools
        psql: {
            capabilities: ["database"]
        }
        mysql: {
            capabilities: ["database"]
        }
        mongosh: {
            capabilities: ["database"]
        }

        // CI/CD tools
        gh: {
            capabilities: ["github"]
        }
        glab: {
            capabilities: ["gitlab"]
        }

        // Container tools
        docker: {
            capabilities: ["docker"]
        }
        kubectl: {
            capabilities: ["kubernetes"]
        }
    }
}
```

## Multiple Capabilities

Variables can require multiple capabilities:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Requires either capability
    MONITORING_KEY: "key" @capability("monitoring", "observability")

    // Admin override - sensitive operations
    ADMIN_TOKEN: "token" @capability("admin", "sensitive")
    DELETE_ENABLED: true @capability("admin", "dangerous")
}
```

## Practical Examples

### Development vs Production

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Development credentials (less restrictive)
    DEV_API_KEY: "dev-key-123" @capability("dev")

    // Production credentials (more restrictive)
    PROD_API_KEY: cuenv.#OnePasswordRef & {ref: "op://Production/API/key"} @capability("prod", "sensitive")

    // Different databases per capability
    DATABASE_URL: "postgres://localhost/dev" @capability("dev")
    DATABASE_URL: "postgres://prod-db/app" @capability("prod")
}
```

### CI/CD Pipeline

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Build tools
    NPM_TOKEN: "token" @capability("build")
    DOCKER_REGISTRY: "registry.example.com" @capability("build")

    // Deployment credentials
    DEPLOY_KEY: cuenv.#OnePasswordRef & {ref: "op://Deploy/SSH/key"} @capability("deploy")
    KUBE_CONFIG: cuenv.#OnePasswordRef & {ref: "op://Deploy/Kubernetes/config"} @capability("deploy")

    // Test environment
    TEST_DATABASE: "postgres://test-db/test" @capability("test")
    TEST_API_KEY: "test-key" @capability("test")

    Commands: {
        npm: {
            capabilities: ["build"]
        }
        docker: {
            capabilities: ["build"]
        }
        kubectl: {
            capabilities: ["deploy"]
        }
        jest: {
            capabilities: ["test"]
        }
    }
}
```

### Multi-Cloud Setup

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // AWS credentials
    AWS_ACCESS_KEY: "key" @capability("aws")
    AWS_SECRET_KEY: "secret" @capability("aws")

    // GCP credentials
    GOOGLE_APPLICATION_CREDENTIALS: "/path/to/key.json" @capability("gcp")
    GCP_PROJECT: "my-project" @capability("gcp")

    // Azure credentials
    AZURE_CLIENT_ID: "id" @capability("azure")
    AZURE_CLIENT_SECRET: "secret" @capability("azure")

    Commands: {
        terraform: {
            capabilities: ["aws", "gcp", "azure"]
        }
        aws: {
            capabilities: ["aws"]
        }
        gcloud: {
            capabilities: ["gcp"]
        }
        az: {
            capabilities: ["azure"]
        }
    }
}
```

## Security Best Practices

### 1. Principle of Least Privilege

Only expose credentials when absolutely necessary:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Bad: No capability restriction
    DATABASE_PASSWORD: "secret123"

    // Good: Restricted to database operations
    DATABASE_PASSWORD: cuenv.#OnePasswordRef & {ref: "op://Vault/Database/password"} @capability("database")
}
```

### 2. Sensitive Data Isolation

Use specific capabilities for sensitive operations:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Regular operations
    API_ENDPOINT: "https://api.example.com"

    // Sensitive operations
    API_ADMIN_KEY: "admin-key" @capability("admin", "sensitive")
    DELETE_ALL_ENDPOINT: "/api/delete-all" @capability("dangerous")
}
```

### 3. Environment-Specific Capabilities

Different capabilities for different environments:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    environment: {
        development: {
            // Developers get more capabilities
            DB_ADMIN_USER: "admin" @capability("dev")
            DEBUG_ENABLED: true @capability("dev")
        }
        production: {
            // Production is locked down
            DB_ADMIN_USER: cuenv.#OnePasswordRef & {ref: "op://Prod/DB/admin"} @capability("prod-admin")
            DEBUG_ENABLED: false
        }
    }
}
```

## Advanced Patterns

### Capability Inheritance

Create capability hierarchies:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Base credentials
    API_READ_KEY: "read-key" @capability("api-read")
    API_WRITE_KEY: "write-key" @capability("api-write")
    API_ADMIN_KEY: "admin-key" @capability("api-admin")

    Commands: {
        // Read-only commands
        "api-client": {
            capabilities: ["api-read"]
        }

        // Write commands get read + write
        "api-sync": {
            capabilities: ["api-read", "api-write"]
        }

        // Admin commands get everything
        "api-admin": {
            capabilities: ["api-read", "api-write", "api-admin"]
        }
    }
}
```

### Dynamic Capability Assignment

Use CUE's power for dynamic capabilities:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Define capability groups
    #DevCapabilities: ["database", "redis", "debug"]
    #ProdCapabilities: ["database", "redis"]

    // Assign based on environment
    _capabilities: {
        if ENVIRONMENT == "development" { #DevCapabilities }
        if ENVIRONMENT == "production" { #ProdCapabilities }
    }

    // Apply to commands
    Commands: {
        "app-server": {
            capabilities: _capabilities
        }
    }
}
```

### Capability Auditing

Track capability usage:

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Tag with audit requirements
    PAYMENT_API_KEY: "key" @capability("payment", "audit-required")
    PII_DATABASE_URL: "url" @capability("pii", "audit-required")

    // Document capability purpose
    _capabilityDocs: {
        payment: "Access to payment processing systems"
        pii: "Access to personally identifiable information"
        audit_required: "All access is logged for compliance"
    }
}
```

## Debugging Capabilities

### Check Active Capabilities

```bash
# See which capabilities are active
CUENV_DEBUG=1 cuenv run -c aws -- env | grep CUENV

# Test capability filtering
cuenv run -- env | grep AWS  # No AWS variables
cuenv run -c aws -- env | grep AWS  # AWS variables visible
```

### List Required Capabilities

Create a helper script to document capabilities:

```bash
#!/bin/bash
# list-capabilities.sh

echo "=== Variables by Capability ==="
grep -E '@capability\(' env.cue | while read -r line; do
    var=$(echo "$line" | cut -d: -f1 | tr -d ' ')
    caps=$(echo "$line" | grep -oE '@capability\([^)]+\)')
    echo "$var: $caps"
done

echo -e "\n=== Command Mappings ==="
grep -A1 "Commands:" env.cue
```

## Common Use Cases

### 1. Microservices

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Service A credentials
    SERVICE_A_API_KEY: "key-a" @capability("service-a")
    SERVICE_A_URL: "http://service-a:8080" @capability("service-a")

    // Service B credentials
    SERVICE_B_API_KEY: "key-b" @capability("service-b")
    SERVICE_B_URL: "http://service-b:8081" @capability("service-b")

    // Gateway needs access to all services
    Commands: {
        "api-gateway": {
            capabilities: ["service-a", "service-b"]
        }
        "service-a-worker": {
            capabilities: ["service-a"]
        }
        "service-b-worker": {
            capabilities: ["service-b"]
        }
    }
}
```

### 2. Third-Party Integrations

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Payment providers
    STRIPE_API_KEY: "sk_live_..." @capability("stripe")
    PAYPAL_CLIENT_ID: "..." @capability("paypal")

    // Communication
    TWILIO_AUTH_TOKEN: "..." @capability("twilio")
    SENDGRID_API_KEY: "..." @capability("sendgrid")

    // Analytics
    SEGMENT_WRITE_KEY: "..." @capability("analytics")
    MIXPANEL_TOKEN: "..." @capability("analytics")
}
```

### 3. Infrastructure as Code

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Cloud credentials
    AWS_ACCESS_KEY: "..." @capability("aws-infra")
    TF_VAR_region: "us-east-1" @capability("terraform")

    // State management
    TF_BACKEND_BUCKET: "terraform-state" @capability("terraform")
    TF_BACKEND_KEY: "prod/terraform.tfstate" @capability("terraform")

    Commands: {
        terraform: {
            capabilities: ["terraform", "aws-infra"]
        }
        terragrunt: {
            capabilities: ["terraform", "aws-infra"]
        }
    }
}
```
