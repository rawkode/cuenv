---
title: Quick Start
description: From zero to typed environment variables in 2 minutes
---

Skip the theory. Here's how to actually use this thing.

## Step 1: Create Your First CUE Package

Create a new directory for your project and add CUE files:

```bash
mkdir my-project
cd my-project
```

Create an env.cue file with your favorite editor:

```cue title="env.cue"
package env

// Application configuration
APP_NAME: "My Awesome App"
APP_ENV: "development"
PORT: 3000

// Database configuration
DATABASE_HOST: "localhost"
DATABASE_PORT: 5432
DATABASE_NAME: "myapp_dev"
DATABASE_USER: "myapp"

// API keys (use secrets in production!)
API_KEY: "dev-api-key-12345"

// Feature flags
ENABLE_DEBUG: true
ENABLE_CACHE: false

// Computed values using CUE
DATABASE_URL: "postgres://\(DATABASE_USER)@\(DATABASE_HOST):\(DATABASE_PORT)/\(DATABASE_NAME)"
```

## Step 2: Allow the Directory

For security, you need to explicitly allow cuenv to load environments. Run `cuenv allow` in your project directory:

```bash
cuenv allow
```

This only needs to be done once for each directory you trust.

## Step 3: See It in Action

Navigate out and back into the directory to see cuenv load your environment:

```bash
# Leave the directory
cd ..

# Check that variables are not set
echo $APP_NAME
# (empty)

# Enter the directory
cd my-project

# Variables are now loaded!
echo $APP_NAME
# Output: My Awesome App

echo $DATABASE_URL
# Output: postgres://myapp@localhost:5432/myapp_dev
```

## Step 3: Multiple CUE Files

You can split your configuration across multiple CUE files in the same package:

```bash
# Create a database configuration file
cat > database.cue << 'EOF'
package env

// Database-specific settings
DATABASE_POOL_SIZE: 10
DATABASE_TIMEOUT: 30
DATABASE_SSL: true
EOF

# Create an app configuration file
cat > app.cue << 'EOF'
package env

// App-specific settings
APP_VERSION: "1.0.0"
APP_TIMEOUT: 60
EOF
```

All files in the package are loaded together:

```bash
echo $DATABASE_POOL_SIZE
# Output: 10

echo $APP_VERSION
# Output: 1.0.0
```

### Automatic File Watching

cuenv automatically watches all loaded CUE files for changes:

```bash
# Edit any CUE file
echo 'NEW_VAR: "added"' >> env.cue

# The environment automatically reloads!
echo $NEW_VAR
# Output: added
```

## Step 4: Using Secrets (Production)

For production environments, use secret references instead of hardcoded values:

```cue
package env

import "github.com/rawkode/cuenv"

// ... other config ...

// 1Password secret reference
DATABASE_PASSWORD: cuenv.#OnePasswordRef & {ref: "op://Personal/myapp-db/password"}

// GCP Secret Manager reference
API_SECRET: cuenv.#GCPSecretRef & {ref: "gcp-secret://my-project/api-secret-key"}

// Composed with resolved secrets
DATABASE_URL: "postgres://\(DATABASE_USER):\(DATABASE_PASSWORD)@\(DATABASE_HOST):\(DATABASE_PORT)/\(DATABASE_NAME)"
```

Run commands with resolved secrets:

```bash
# Secrets are resolved only when using 'cuenv run'
cuenv run node app.js

# Regular shell use won't resolve secrets (for security)
echo $DATABASE_PASSWORD
# Output: cuenv.#OnePasswordRef & {ref: "op://Personal/myapp-db/password"}
```

## Step 5: Environment-Specific Configuration

Create environment-specific overrides:

```cue
package env

// Base configuration
APP_NAME: "My App"
PORT: 3000
LOG_LEVEL: "info"

// Environment-specific overrides
environment: {
    production: {
        PORT: 8080
        LOG_LEVEL: "error"
        DATABASE_HOST: "prod-db.example.com"
    }
    staging: {
        PORT: 3001
        LOG_LEVEL: "debug"
        DATABASE_HOST: "staging-db.example.com"
    }
}
```

Use different environments:

```bash
# Development (default)
cuenv run -- echo $PORT
# Output: 3000

# Production
cuenv run -e production -- echo $PORT
# Output: 8080

# Or use environment variable
CUENV_ENV=staging cuenv run -- echo $DATABASE_HOST
# Output: staging-db.example.com
```

## Common Patterns

### Shell Variable Expansion

```cue
package env

// Use $HOME and other shell variables
LOG_PATH: "$HOME/logs/myapp"
CONFIG_PATH: "${HOME}/.config/myapp"
```

### Boolean Values

```cue
package env

// Booleans are converted to "true"/"false" strings
ENABLE_FEATURE: true
DEBUG_MODE: false
```

### Number Values

```cue
package env

// Numbers are converted to strings
TIMEOUT: 30
MAX_CONNECTIONS: 100
PORT: 8080
```

### Using CUE Constraints

```cue
package env

// Set defaults with constraints
ENVIRONMENT: *"development" | "staging" | "production"
PORT: int & >=1024 & <=65535 | *3000
```

## Next Steps

- Learn more about [CUE file format](/guides/cue-format/)
- Set up [secret management](/guides/secrets/)
- Configure [multiple environments](/guides/environments/)
- Explore [command reference](/reference/commands/)
