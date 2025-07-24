---
title: CUE File Format
description: Learn how to write CUE configuration files for cuenv
---

CUE (Configure, Unify, Execute) is a powerful configuration language that provides type safety and validation. This guide covers how to use CUE with cuenv.

## Basic Syntax

Every cuenv configuration file must:
1. Be named `env.cue`
2. Declare `package env`
3. Define environment variables as top-level fields

```cue title="env.cue"
package env

// String values
APP_NAME: "My Application"
DATABASE_URL: "postgres://localhost/mydb"

// Number values
PORT: 3000
TIMEOUT: 30

// Boolean values
DEBUG: true
ENABLE_CACHE: false
```

## Data Types

### Strings

```cue title="env.cue"
package env

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
```

### Numbers

```cue title="env.cue"
package env

// Integers
PORT: 8080
MAX_CONNECTIONS: 100
RETRY_COUNT: 3

// Floats (converted to string)
VERSION: 1.5
THRESHOLD: 0.95
```

### Booleans

```cue title="env.cue"
package env

// Booleans are converted to "true" or "false" strings
ENABLE_DEBUG: true
USE_CACHE: false
IS_PRODUCTION: false
```

## CUE Features

### String Interpolation

```cue title="env.cue"
package env

// Basic interpolation
HOST: "localhost"
PORT: 5432
DATABASE: "myapp"
DATABASE_URL: "postgres://\(HOST):\(PORT)/\(DATABASE)"

// Complex interpolation
USER: "admin"
DOMAIN: "example.com"
EMAIL: "\(USER)@\(DOMAIN)"
```

### Computed Values

```cue title="env.cue"
package env

// Mathematical operations
BASE_PORT: 3000
METRICS_PORT: BASE_PORT + 1
DEBUG_PORT: BASE_PORT + 2

// String operations
APP_PREFIX: "myapp"
CACHE_KEY: "\(APP_PREFIX)_cache"
QUEUE_NAME: "\(APP_PREFIX)_queue"
```

### Constraints and Defaults

```cue title="env.cue"
package env

// Default values with constraints
ENVIRONMENT: *"development" | "staging" | "production"
LOG_LEVEL: *"info" | "debug" | "warn" | "error"

// Numeric constraints
PORT: int & >=1024 & <=65535 | *3000
WORKERS: int & >=1 & <=100 | *4

// String constraints
REGION: "us-east-1" | "us-west-2" | "eu-west-1" | *"us-east-1"
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
DB_HOST: _db.host
DB_PORT: _db.port
DB_NAME: _db.name
DB_USER: _db.user
```

## Shell Variable Expansion

cuenv supports shell variable expansion in string values:

```cue title="env.cue"
package env

// Using $HOME
LOG_DIR: "$HOME/logs"
CONFIG_PATH: "$HOME/.config/myapp"

// Using ${} syntax
CACHE_DIR: "${HOME}/.cache/myapp"
DATA_PATH: "${HOME}/data/${APP_NAME}"

// Escaped dollar signs
PRICE: "\\$99.99"
TEMPLATE: "User: \\${username}"
```

## Advanced Patterns

### Conditional Values

```cue title="env.cue"
package env

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
```

### Lists and Joining

```cue title="env.cue"
package env

// Define lists (CUE internal use)
_features: ["auth", "api", "cache", "queue"]

// Join into environment variable
ENABLED_FEATURES: strings.Join(_features, ",")

// Or use explicit string
ALLOWED_ORIGINS: "https://example.com,https://app.example.com"
```

### Importing CUE Packages

```cue title="env.cue"
package env

import "strings"

// Use CUE's built-in packages
APP_NAME: "my-app"
APP_NAME_UPPER: strings.ToUpper(APP_NAME)
APP_NAME_TITLE: strings.ToTitle(APP_NAME)
```

## Best Practices

### 1. Use Meaningful Names

```cue title="env.cue"
package env

// Good: Clear and descriptive
DATABASE_CONNECTION_TIMEOUT: 30
API_RATE_LIMIT_PER_MINUTE: 100

// Avoid: Too generic
TIMEOUT: 30
LIMIT: 100
```

### 2. Group Related Variables

```cue title="env.cue"
package env

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
```

### 3. Document Complex Values

```cue title="env.cue"
package env

// JWT expiration in seconds (24 hours)
JWT_EXPIRATION: 86400

// Maximum file upload size in MB
MAX_UPLOAD_SIZE: 10

// Cache TTL in seconds (5 minutes)
CACHE_TTL: 300
```

### 4. Use Type Constraints

```cue title="env.cue"
package env

// Ensure valid port numbers
PORT: int & >=1024 & <=65535 | *3000

// Ensure valid percentages
CPU_THRESHOLD: float & >=0.0 & <=1.0 | *0.8

// Ensure specific string values
LOG_FORMAT: "json" | "text" | *"json"
```

## Common Patterns

### Feature Flags

```cue title="env.cue"
package env

// Boolean feature flags
FEATURE_NEW_UI: true
FEATURE_BETA_API: false
FEATURE_ANALYTICS: true

// String-based feature flags
FEATURE_LEVEL: "basic" | "premium" | "enterprise" | *"basic"
```

### Multi-Environment Setup

```cue title="env.cue"
package env

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
```

### URL Construction

```cue title="env.cue"
package env

// Build URLs from components
API_PROTOCOL: "https"
API_HOST: "api.example.com"
API_VERSION: "v2"
API_BASE_URL: "\(API_PROTOCOL)://\(API_HOST)/\(API_VERSION)"

// Construct full endpoints
USER_ENDPOINT: "\(API_BASE_URL)/users"
AUTH_ENDPOINT: "\(API_BASE_URL)/auth"
```

## Troubleshooting

### Common Errors

1. **Missing package declaration**
   ```cue
   // Error: Missing package declaration
   PORT: 3000
   
   // Correct: Include package env
   package env
   PORT: 3000
   ```

2. **Invalid type mixing**
   ```cue
   package env
   
   // Error: Can't add string to number
   PORT: 3000
   DEBUG_URL: "localhost:" + PORT
   
   // Correct: Use interpolation
   DEBUG_URL: "localhost:\(PORT)"
   ```

3. **Undefined references**
   ```cue
   package env
   
   // Error: HOSTNAME is not defined
   URL: "https://\(HOSTNAME)/api"
   
   // Correct: Define HOSTNAME first
   HOSTNAME: "example.com"
   URL: "https://\(HOSTNAME)/api"
   ```