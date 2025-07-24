---
title: Managing Environments
description: Learn how to manage multiple environments with cuenv
---

cuenv provides powerful features for managing different environments (development, staging, production) with environment-specific configurations and capability-based filtering.

## Environment Configuration

### Basic Environment Setup

Define environment-specific overrides in your `env.cue`:

```cue title="env.cue"
package env

// Base configuration - applies to all environments
APP_NAME: "myapp"
LOG_LEVEL: "info"
PORT: 3000
DATABASE_HOST: "localhost"

// Environment-specific overrides
environment: {
    development: {
        LOG_LEVEL: "debug"
        DEBUG: true
        DATABASE_HOST: "localhost"
    }
    staging: {
        LOG_LEVEL: "info"
        PORT: 3001
        DATABASE_HOST: "staging-db.internal"
    }
    production: {
        LOG_LEVEL: "error"
        PORT: 8080
        DATABASE_HOST: "prod-db.internal"
        DEBUG: false
    }
}
```

### Using Environments

There are three ways to specify which environment to use:

1. **Command-line flag:**
   ```bash
   cuenv run -e production -- node server.js
   ```

2. **Environment variable:**
   ```bash
   CUENV_ENV=production cuenv run -- node server.js
   ```

3. **Default environment (no flag):**
   ```bash
   cuenv run -- node server.js  # Uses base configuration
   ```

## Environment Inheritance

Environment configurations inherit and override base values:

```cue title="env.cue"
package env

// Base values
API_URL: "http://localhost:3000"
CACHE_TTL: 300
WORKERS: 4

environment: {
    production: {
        // Override specific values
        API_URL: "https://api.example.com"
        WORKERS: 16
        // CACHE_TTL remains 300 (inherited)
        
        // Add production-only values
        ENABLE_MONITORING: true
        SENTRY_DSN: "https://key@sentry.io/project"
    }
}
```

## Complex Environment Patterns

### Environment-Specific Secrets

```cue title="env.cue"
package env

APP_NAME: "myapp"

environment: {
    development: {
        DATABASE_URL: "postgres://localhost/myapp_dev"
        API_KEY: "dev-key-12345"  // Hardcoded for dev
    }
    staging: {
        DATABASE_URL: "op://Staging/Database/url"
        API_KEY: "op://Staging/API/key"
    }
    production: {
        DATABASE_URL: "op://Production/Database/url"
        API_KEY: "gcp-secret://prod-project/api-key"
    }
}
```

### Feature Flags by Environment

```cue title="env.cue"
package env

// Default feature flags
FEATURE_NEW_UI: false
FEATURE_BETA_API: false

environment: {
    development: {
        // Enable all features in dev
        FEATURE_NEW_UI: true
        FEATURE_BETA_API: true
        FEATURE_DEBUG_PANEL: true
    }
    staging: {
        // Test new features in staging
        FEATURE_NEW_UI: true
        FEATURE_BETA_API: false
    }
    production: {
        // Conservative production settings
        FEATURE_NEW_UI: false
        FEATURE_BETA_API: false
    }
}
```

### Environment-Specific Services

```cue title="env.cue"
package env

// Service endpoints vary by environment
environment: {
    development: {
        REDIS_URL: "redis://localhost:6379"
        ELASTICSEARCH_URL: "http://localhost:9200"
        S3_BUCKET: "myapp-dev"
    }
    staging: {
        REDIS_URL: "redis://redis.staging.internal:6379"
        ELASTICSEARCH_URL: "https://es.staging.internal:9200"
        S3_BUCKET: "myapp-staging"
    }
    production: {
        REDIS_URL: "redis://redis-cluster.prod.internal:6379"
        ELASTICSEARCH_URL: "https://es-cluster.prod.internal:9200"
        S3_BUCKET: "myapp-production"
    }
}
```

## Capability-Based Filtering

Capabilities allow you to control which environment variables are exposed based on the command being run.

### Defining Capabilities

```cue title="env.cue"
package env

// Tag variables with capabilities
AWS_ACCESS_KEY: "key" @capability("aws")
AWS_SECRET_KEY: "secret" @capability("aws")
AWS_REGION: "us-east-1" @capability("aws")

GITHUB_TOKEN: "token" @capability("github")
GITHUB_ORG: "myorg" @capability("github")

DATABASE_URL: "postgres://..." @capability("database")
```

### Command Mapping

Define which commands automatically get which capabilities:

```cue title="env.cue"
package env

// Map commands to their required capabilities
Commands: {
    terraform: capabilities: ["aws", "cloudflare"]
    aws: capabilities: ["aws"]
    gh: capabilities: ["github"]
    psql: capabilities: ["database"]
    mysql: capabilities: ["database"]
}
```

### Using Capabilities

1. **Explicit capabilities:**
   ```bash
   cuenv run -c aws,github -- terraform plan
   ```

2. **Automatic inference:**
   ```bash
   # Automatically gets 'aws' capability
   cuenv run -- aws s3 ls
   ```

3. **Environment variable:**
   ```bash
   CUENV_CAPABILITIES=aws,database cuenv run -- ./deploy.sh
   ```

## Practical Examples

### Multi-Region Deployment

```cue title="env.cue"
package env

// Base configuration
APP_NAME: "myapp"
AWS_REGION: "us-east-1"

environment: {
    "production-us": {
        AWS_REGION: "us-east-1"
        API_ENDPOINT: "https://api-us.example.com"
        DATABASE_REGION: "us-east-1"
    }
    "production-eu": {
        AWS_REGION: "eu-west-1"
        API_ENDPOINT: "https://api-eu.example.com"
        DATABASE_REGION: "eu-west-1"
    }
    "production-asia": {
        AWS_REGION: "ap-southeast-1"
        API_ENDPOINT: "https://api-asia.example.com"
        DATABASE_REGION: "ap-southeast-1"
    }
}
```

Usage:
```bash
# Deploy to US region
cuenv run -e production-us -- ./deploy.sh

# Deploy to EU region
cuenv run -e production-eu -- ./deploy.sh
```

### Development Modes

```cue title="env.cue"
package env

// Different development configurations
environment: {
    "dev-local": {
        DATABASE_URL: "postgres://localhost/myapp_dev"
        REDIS_URL: "redis://localhost:6379"
        USE_LOCAL_STORAGE: true
    }
    "dev-docker": {
        DATABASE_URL: "postgres://db:5432/myapp_dev"
        REDIS_URL: "redis://redis:6379"
        USE_LOCAL_STORAGE: false
    }
    "dev-remote": {
        DATABASE_URL: "op://Development/Remote-DB/url"
        REDIS_URL: "redis://dev.redis.internal:6379"
        USE_LOCAL_STORAGE: false
    }
}
```

### CI/CD Environments

```cue title="env.cue"
package env

environment: {
    ci: {
        NODE_ENV: "test"
        DATABASE_URL: "postgres://postgres@localhost/test"
        DISABLE_AUTH: true
        LOG_LEVEL: "error"
        // CI-specific tokens
        CODECOV_TOKEN: "gcp-secret://ci-project/codecov-token"
    }
    cd: {
        // Deployment environment
        DEPLOY_KEY: "op://DevOps/Deploy-Key/private"
        DOCKER_REGISTRY: "gcr.io/my-project"
        KUBECTL_CONTEXT: "production-cluster"
    }
}
```

## Best Practices

### 1. Environment Naming Conventions

Use clear, consistent naming:
- `development` / `dev`
- `staging` / `stage`
- `production` / `prod`
- `testing` / `test`

For multi-region:
- `production-us-east-1`
- `production-eu-west-1`
- `staging-us-east-1`

### 2. Environment Validation

Use CUE constraints to validate environment values:

```cue title="env.cue"
package env

// Ensure production has required values
environment: production: {
    // These fields are required in production
    DATABASE_URL: string
    API_KEY: string
    SENTRY_DSN: string
    
    // Ensure specific values
    NODE_ENV: "production"
    DEBUG: false
}
```

### 3. Sensitive Data Handling

Never hardcode secrets in production environments:

```cue title="env.cue"
package env

environment: {
    development: {
        // OK for development
        API_KEY: "dev-key-12345"
    }
    production: {
        // Always use secret references
        API_KEY: "op://Production/API/key"
    }
}
```

### 4. Environment Documentation

Document environment-specific behavior:

```cue title="env.cue"
package env

environment: {
    development: {
        // Sends emails to mailcatcher on port 1025
        SMTP_HOST: "localhost"
        SMTP_PORT: 1025
    }
    production: {
        // Uses SendGrid for production emails
        SMTP_HOST: "smtp.sendgrid.net"
        SMTP_PORT: 587
    }
}
```

## Advanced Patterns

### Dynamic Environment Selection

Use CUE's power for dynamic configuration:

```cue title="env.cue"
package env

import "strings"

// Detect environment from hostname
_hostname: string | *"dev-machine" // Would be set externally

_env: {
    if strings.HasPrefix(_hostname, "prod-") {
        "production"
    }
    if strings.HasPrefix(_hostname, "staging-") {
        "staging"
    }
    if true {
        "development"
    }
}

// Apply detected environment
if _env == "production" {
    LOG_LEVEL: "error"
    DEBUG: false
}
```

### Environment Composition

Compose environments from multiple sources:

```cue title="env.cue"
package env

// Shared configurations
#BaseConfig: {
    APP_NAME: "myapp"
    TIMEZONE: "UTC"
}

#AWSConfig: {
    AWS_REGION: string
    AWS_DEFAULT_REGION: AWS_REGION
}

#DatabaseConfig: {
    DATABASE_POOL_MIN: 2
    DATABASE_POOL_MAX: 10
}

// Compose environments
environment: production: #BaseConfig & #AWSConfig & #DatabaseConfig & {
    AWS_REGION: "us-east-1"
    DATABASE_POOL_MAX: 50  // Override default
}
```

### Environment Aliases

Create aliases for common environment combinations:

```cue title="env.cue"
package env

// Define base environments
environment: {
    "prod-us": { /* ... */ }
    "prod-eu": { /* ... */ }
    
    // Aliases
    "prod": "prod-us"  // Default production is US
    "p": "prod-us"     // Short alias
}
```

## Testing Different Environments

### Local Testing

Test different environments locally:

```bash
# Test with production configuration
cuenv run -e production -- npm test

# Compare outputs across environments
for env in development staging production; do
    echo "Environment: $env"
    cuenv run -e $env -- node -e 'console.log(process.env.DATABASE_URL)'
done
```

### Validation Script

Create a script to validate environment configurations:

```bash
#!/bin/bash
# validate-envs.sh

REQUIRED_VARS="DATABASE_URL API_KEY LOG_LEVEL"

for env in development staging production; do
    echo "Validating $env..."
    for var in $REQUIRED_VARS; do
        value=$(cuenv run -e $env -- sh -c "echo \$$var")
        if [ -z "$value" ]; then
            echo "  ERROR: $var is not set"
        else
            echo "  OK: $var is set"
        fi
    done
done
```