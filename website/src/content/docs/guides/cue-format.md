---
title: CUE File Format
description: Like JSON but with actual features
---

CUE is what JSON should have been. Types, validation, no trailing comma drama. Here's how to use it with cuenv.

## The Basics

Your `env.cue` needs:

1. `package env` at the top
2. Variables as key-value pairs inside an `env:` field
3. That's it

Variables go inside the `env:` field, and tasks (if any) go at the top level in a `tasks:` field.

```cue title="env.cue"
package env

env: {
    // String values
    APP_NAME: "My Application"
    DATABASE_URL: "postgres://localhost/mydb"

    // Number values
    PORT: 3000
    TIMEOUT: 30

    // Boolean values
    DEBUG: true
    ENABLE_CACHE: false
}
```

## Data Types

### Strings

```cue title="env.cue"
package env

env: {
    // Simple strings
    NAME: "John Doe"
    MESSAGE: "Hello, World!"

    // Multi-line strings
    DESCRIPTION: """
        This is a multi-line
        string value that preserves
        line breaks.
        """

    // String with quotes
    QUOTED: "She said \"Hello\""
}
```

### Numbers

```cue title="env.cue"
package env

env: {
    // Integers
    PORT: 8080
    MAX_CONNECTIONS: 100
    RETRY_COUNT: 3

    // Floats (converted to string)
    VERSION: 1.5
    THRESHOLD: 0.95
}
```

### Booleans

```cue title="env.cue"
package env

env: {
    // Booleans are converted to "true" or "false" strings
    ENABLE_DEBUG: true
    USE_CACHE: false
    IS_PRODUCTION: false
}
```

## CUE Features

### String Interpolation

```cue title="env.cue"
package env

env: {
    // Basic interpolation
    HOST: "localhost"
    PORT: 5432
    DATABASE: "myapp"
    DATABASE_URL: "postgres://\(HOST):\(PORT)/\(DATABASE)"

    // Complex interpolation
    USER: "admin"
    DOMAIN: "example.com"
    EMAIL: "\(USER)@\(DOMAIN)"
}
```

### Computed Values

```cue title="env.cue"
package env

env: {
    // Mathematical operations
    BASE_PORT: 3000
    METRICS_PORT: BASE_PORT + 1
    DEBUG_PORT: BASE_PORT + 2

    // String operations
    APP_PREFIX: "myapp"
    CACHE_KEY: "\(APP_PREFIX)_cache"
    QUEUE_NAME: "\(APP_PREFIX)_queue"
}
```

### Constraints and Defaults

```cue title="env.cue"
package env

env: {
    // Default values with constraints
    ENVIRONMENT: *"development" | "staging" | "production"
    LOG_LEVEL: *"info" | "debug" | "warn" | "error"

    // Numeric constraints
    PORT: int & >=1024 & <=65535 | *3000
    WORKERS: int & >=1 & <=100 | *4

    // String constraints
    REGION: "us-east-1" | "us-west-2" | "eu-west-1" | *"us-east-1"
}
```

### Definitions and Reuse

```cue title="env.cue"
package env

// Define reusable patterns
#DatabaseConfig: {
    host: string
    port: int & >=1 & <=65535
    name: string
    user: string
}

// Use the definition
_db: #DatabaseConfig & {
    host: "localhost"
    port: 5432
    name: "myapp"
    user: "dbuser"
}

// Export as environment variables
env: {
    DB_HOST: _db.host
    DB_PORT: _db.port
    DB_NAME: _db.name
    DB_USER: _db.user
}
```

## Shell Variable Expansion

cuenv supports shell variable expansion in string values:

```cue title="env.cue"
package env

env: {
    // Using $HOME
    LOG_DIR: "$HOME/logs"
    CONFIG_PATH: "$HOME/.config/myapp"

    // Using ${} syntax
    CACHE_DIR: "${HOME}/.cache/myapp"
    DATA_PATH: "${HOME}/data/${APP_NAME}"

    // Escaped dollar signs
    PRICE: "\\$99.99"
    TEMPLATE: "User: \\${username}"
}
```

## Advanced Patterns

### Conditional Values

```cue title="env.cue"
package env

env: {
    ENVIRONMENT: "development"

    // Use CUE's powerful constraint system
    _isDev: ENVIRONMENT == "development"

    // Different values based on environment
    DATABASE_HOST: {
        if _isDev {
            "localhost"
        }
        if !_isDev {
            "prod-db.example.com"
        }
    }
}
```

### Lists and Joining

```cue title="env.cue"
package env

import "strings"

// Define lists (CUE internal use)
_features: ["auth", "api", "cache", "queue"]

env: {
    // Join into environment variable
    ENABLED_FEATURES: strings.Join(_features, ",")

    // Or use explicit string
    ALLOWED_ORIGINS: "https://example.com,https://app.example.com"
}
```

### Importing CUE Packages

```cue title="env.cue"
package env

import "strings"

env: {
    // Use CUE's built-in packages
    APP_NAME: "my-app"
    APP_NAME_UPPER: strings.ToUpper(APP_NAME)
    APP_NAME_TITLE: strings.ToTitle(APP_NAME)
}
```

## Best Practices

### 1. Use Meaningful Names

```cue title="env.cue"
package env

env: {
    // Good: Clear and descriptive
    DATABASE_CONNECTION_TIMEOUT: 30
    API_RATE_LIMIT_PER_MINUTE: 100

    // Avoid: Too generic
    TIMEOUT: 30
    LIMIT: 100
}
```

### 2. Group Related Variables

```cue title="env.cue"
package env

env: {
    // Database configuration
    DATABASE_HOST: "localhost"
    DATABASE_PORT: 5432
    DATABASE_NAME: "myapp"
    DATABASE_USER: "dbuser"

    // Redis configuration
    REDIS_HOST: "localhost"
    REDIS_PORT: 6379
    REDIS_DB: 0

    // API configuration
    API_BASE_URL: "https://api.example.com"
    API_VERSION: "v1"
    API_TIMEOUT: 30
}
```

### 3. Document Complex Values

```cue title="env.cue"
package env

env: {
    // JWT expiration in seconds (24 hours)
    JWT_EXPIRATION: 86400

    // Maximum file upload size in MB
    MAX_UPLOAD_SIZE: 10

    // Cache TTL in seconds (5 minutes)
    CACHE_TTL: 300
}
```

### 4. Use Type Constraints

```cue title="env.cue"
package env

env: {
    // Ensure valid port numbers
    PORT: int & >=1024 & <=65535 | *3000

    // Ensure valid percentages
    CPU_THRESHOLD: float & >=0.0 & <=1.0 | *0.8

    // Ensure specific string values
    LOG_FORMAT: "json" | "text" | *"json"
}
```

## Common Patterns

### Feature Flags

```cue title="env.cue"
package env

env: {
    // Boolean feature flags
    FEATURE_NEW_UI: true
    FEATURE_BETA_API: false
    FEATURE_ANALYTICS: true

    // String-based feature flags
    FEATURE_LEVEL: "basic" | "premium" | "enterprise" | *"basic"
}
```

### Multi-Environment Setup

```cue title="env.cue"
package env

import "github.com/rawkode/cuenv"

env: cuenv.#Env & {
    // Base configuration
    APP_NAME: "myapp"
    LOG_LEVEL: "info"

    // Environment-specific overrides
    environment: {
        development: {
            LOG_LEVEL: "debug"
            DEBUG: true
        }
        production: {
            LOG_LEVEL: "error"
            DEBUG: false
        }
    }
}
```

### URL Construction

```cue title="env.cue"
package env

env: {
    // Build URLs from components
    API_PROTOCOL: "https"
    API_HOST: "api.example.com"
    API_VERSION: "v2"
    API_BASE_URL: "\(API_PROTOCOL)://\(API_HOST)/\(API_VERSION)"

    // Construct full endpoints
    USER_ENDPOINT: "\(API_BASE_URL)/users"
    AUTH_ENDPOINT: "\(API_BASE_URL)/auth"
}
```

## Troubleshooting

### Common Errors

1. **Missing package declaration**

   ```cue
   // Error: Missing package declaration
   env: {
       PORT: 3000
   }

   // Correct: Include package env
   package env

   env: {
       PORT: 3000
   }
   ```

1. **Invalid type mixing**

   ```cue
   package env

   env: {
       // Error: Can't add string to number
       PORT: 3000
       DEBUG_URL: "localhost:" + PORT

       // Correct: Use interpolation
       DEBUG_URL: "localhost:\(PORT)"
   }
   ```

1. **Undefined references**

   ```cue
   package env

   env: {
       // Error: HOSTNAME is not defined
       URL: "https://\(HOSTNAME)/api"

       // Correct: Define HOSTNAME first
       HOSTNAME: "example.com"
       URL: "https://\(HOSTNAME)/api"
   }
   ```

## Tasks

cuenv supports defining tasks that can be executed with the `cuenv task` command. Tasks are defined at the top level of your `env.cue` file in a `tasks:` field (not inside the `env:` field).

### Basic Task Definition

```cue title="env.cue"
package env

env: {
    // Your environment variables
    APP_NAME: "myapp"
    NODE_ENV: "development"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "npm run build"
    }

    "test": {
        description: "Run tests"
        command: "npm test"
    }

    "deploy": {
        description: "Deploy to production"
        script: """
            echo "Building application..."
            npm run build
            echo "Running tests..."
            npm test
            echo "Deploying..."
            npm run deploy:prod
            """
    }
}
```

### Task Properties

Tasks support the following properties:

- `description`: A brief description of what the task does
- `command`: A single command to execute (mutually exclusive with `script`)
- `script`: A multi-line script to execute (mutually exclusive with `command`)
- `dependencies`: An array of task names that must run before this task
- `workingDir`: The directory to execute the task in
- `shell`: The shell to use for execution (defaults to system shell)
- `inputs`: Array of file patterns that trigger task re-execution
- `outputs`: Array of file patterns produced by the task

### Task Dependencies

```cue title="env.cue"
package env

tasks: {
    "clean": {
        description: "Clean build artifacts"
        command: "rm -rf dist/"
    }

    "build": {
        description: "Build the project"
        command: "npm run build"
        dependencies: ["clean"]
    }

    "test": {
        description: "Run tests"
        command: "npm test"
        dependencies: ["build"]
    }

    "deploy": {
        description: "Deploy to production"
        command: "npm run deploy"
        dependencies: ["test"]
    }
}
```

### Advanced Task Configuration

```cue title="env.cue"
package env

tasks: {
    "generate-docs": {
        description: "Generate API documentation"
        command: "typedoc"
        workingDir: "./src"
        inputs: ["src/**/*.ts"]
        outputs: ["docs/**/*.html"]
    }

    "docker-build": {
        description: "Build Docker image"
        script: """
            docker build -t myapp:latest .
            docker tag myapp:latest myapp:$(git rev-parse --short HEAD)
            """
        shell: "/bin/bash"
    }

    "ci": {
        description: "Run full CI pipeline"
        dependencies: ["lint", "test", "build"]
    }
}
```

### Running Tasks

Execute tasks using the `cuenv task` command:

```bash
# Run a specific task
cuenv task build

# List all available tasks
cuenv task

# Run a task with dependencies
cuenv task deploy  # Will run clean, build, test, then deploy
```

### Task Environment

Tasks inherit all environment variables defined in your `env:` field, making it easy to use configuration values in your scripts:

```cue title="env.cue"
package env

env: {
    API_URL: "https://api.example.com"
    API_KEY: "secret-key"
    ENVIRONMENT: "production"
}

tasks: {
    "health-check": {
        description: "Check API health"
        command: "curl -H 'X-API-Key: $API_KEY' $API_URL/health"
    }

    "backup": {
        description: "Backup database"
        script: """
            if [ "$ENVIRONMENT" = "production" ]; then
                echo "Backing up production database..."
                pg_dump $DATABASE_URL > backup-$(date +%Y%m%d).sql
            else
                echo "Skipping backup in non-production environment"
            fi
            """
    }
}
```

## Hooks

cuenv supports hooks that run when entering or exiting an environment. Hooks must be defined at the top level of your `env.cue` file, not inside the `env:` field:

```cue title="env.cue"
package env

// Hook definitions at top level
hooks: {
    // Hook that runs when entering the environment
    onEnter: {
        command: "echo"
        args: ["🚀 Environment activated! Database: $DATABASE_URL"]
    }

    // Hook that runs when exiting the environment
    onExit: {
        command: "echo"
        args: ["👋 Cleaning up environment..."]
    }
}

// Environment variables in separate env: field
env: {
    // Regular environment variables
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
}
```

Hook properties:

- `command`: The command to execute
- `args`: Array of arguments to pass to the command

Hooks have access to all environment variables defined in the `env:` field.
