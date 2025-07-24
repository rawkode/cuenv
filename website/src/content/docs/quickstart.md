______________________________________________________________________

## title: Quick Start description: Get up and running with cuenv in 5 minutes

This guide will walk you through creating your first cuenv configuration and using it in a project.

## Step 1: Create Your First env.cue

Create a new directory for your project and add an `env.cue` file:

```bash
mkdir my-project
cd my-project
```

Create `env.cue` with your favorite editor:

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

## Step 2: See It in Action

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

## Step 3: Hierarchical Configuration

Create a parent configuration that will be shared across projects:

```bash
# In the parent directory
cd ..
cat > env.cue << 'EOF'
package env

// Shared organizational settings
ORG_NAME: "My Company"
DEFAULT_REGION: "us-east-1"
LOG_LEVEL: "info"
EOF
```

Now your project inherits these values:

```bash
cd my-project
echo $ORG_NAME
# Output: My Company
```

## Step 4: Using Secrets (Production)

For production environments, use secret references instead of hardcoded values:

```cue
package env

// ... other config ...

// 1Password secret reference
DATABASE_PASSWORD: "op://Personal/myapp-db/password"

// GCP Secret Manager reference  
API_SECRET: "gcp-secret://my-project/api-secret-key"

// Composed with resolved secrets
DATABASE_URL: "postgres://\(DATABASE_USER):\(DATABASE_PASSWORD)@\(DATABASE_HOST):\(DATABASE_PORT)/\(DATABASE_NAME)"
```

Run commands with resolved secrets:

```bash
# Secrets are resolved only when using 'cuenv run'
cuenv run node app.js

# Regular shell use won't resolve secrets (for security)
echo $DATABASE_PASSWORD
# Output: op://Personal/myapp-db/password
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
