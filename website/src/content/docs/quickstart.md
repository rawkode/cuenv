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
package cuenv

env: {
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
}
```

## Step 2: Allow the Directory

For security, you need to explicitly allow cuenv to load environments. Run `cuenv env allow` in your project directory:

```bash
cuenv env allow
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
package cuenv

env: {
    // Database-specific settings
    DATABASE_POOL_SIZE: 10
    DATABASE_TIMEOUT: 30
    DATABASE_SSL: true
}
EOF

# Create an app configuration file
cat > app.cue << 'EOF'
package cuenv

env: {
    // App-specific settings
    APP_VERSION: "1.0.0"
    APP_TIMEOUT: 60
}
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

## Step 4: Using Tasks

cuenv also supports defining and executing tasks alongside your environment configuration:

```cue
package cuenv

env: {
    APP_NAME: "My Awesome App"
    PORT: 3000
}

tasks: {
    install: {
        description: "Install dependencies"
        command: "npm install"
    }

    build: {
        description: "Build the application"
        command: "npm run build"
        dependencies: ["install"]
    }

    start: {
        description: "Start the application"
        command: "npm start"
        dependencies: ["build"]
    }
}
```

Execute tasks using the task command:

```bash
# List all available tasks
cuenv task

# Execute a specific task
cuenv task build

# Execute a task with arguments
cuenv task start -- --port 4000
```

## Step 5: Environment-Specific Configuration

You can use CUE's powerful unification to create environment-specific configurations:

```cue
package cuenv

env: {
    // Base configuration
    APP_NAME: "My App"
    PORT: 3000
    LOG_LEVEL: "info"

    // Set environment-specific values based on CUENV_ENV
    if CUENV_ENV == "production" {
        PORT: 8080
        LOG_LEVEL: "error"
        DATABASE_HOST: "prod-db.example.com"
    }

    if CUENV_ENV == "staging" {
        PORT: 3001
        LOG_LEVEL: "debug"
        DATABASE_HOST: "staging-db.example.com"
    }
}
```

Use different environments:

```bash
# Development (default)
cuenv exec -- echo $PORT
# Output: 3000

# Production
cuenv exec -e production -- echo $PORT
# Output: 8080

# Or set environment variable
CUENV_ENV=staging cuenv exec -- echo $DATABASE_HOST
# Output: staging-db.example.com
```

## Common Patterns

### Shell Variable Expansion

```cue
package cuenv

env: {
    // Use $HOME and other shell variables
    LOG_PATH: "$HOME/logs/myapp"
    CONFIG_PATH: "${HOME}/.config/myapp"
}
```

### Boolean Values

```cue
package cuenv

env: {
    // Booleans are converted to "true"/"false" strings
    ENABLE_FEATURE: true
    DEBUG_MODE: false
}
```

### Number Values

```cue
package cuenv

env: {
    // Numbers are converted to strings
    TIMEOUT: 30
    MAX_CONNECTIONS: 100
    PORT: 8080
}
```

### Using CUE Constraints

```cue
package cuenv

env: {
    // Set defaults with constraints
    ENVIRONMENT: *"development" | "staging" | "production"
    PORT: int & >=1024 & <=65535 | *3000
}
```

## Next Steps

- Learn more about [CUE file format](/guides/cue-format/)
- Set up [secret management](/guides/secrets/)
- Configure [multiple environments](/guides/environments/)
- Explore [command reference](/reference/commands/)
