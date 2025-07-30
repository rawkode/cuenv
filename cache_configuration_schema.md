# Cache Configuration Schema

This document defines the complete configuration schema for the improved cuenv cache system, including global settings, per-task overrides, and environment variable support.

## Global Cache Configuration

### Configuration File Format

The global cache configuration can be defined in a JSON file at `~/.config/cuenv/cache.json` or `.cuenv/cache.json` in the project root.

```json
{
	"enabled": true,
	"default_mode": "read-write",
	"cache_dir": "~/.cache/cuenv",
	"max_size": 10737418240,
	"env_include": ["PATH", "HOME", "USER", "SHELL", "LANG", "CUENV_*"],
	"env_exclude": ["RANDOM", "TEMP", "TMP", "TERM", "SSH_*", "DISPLAY"],
	"eviction_policy": "lru",
	"stats_retention_days": 30,
	"remote_cache": {
		"endpoint": "grpc://cache.example.com:9092",
		"auth_token": "${CUENV_CACHE_AUTH_TOKEN}",
		"timeout_seconds": 30,
		"max_concurrent": 10,
		"upload_enabled": true,
		"download_enabled": true
	},
	"storage": {
		"inline_threshold": 4096,
		"compression_enabled": true,
		"integrity_check_enabled": true,
		"gc_interval_seconds": 300
	}
}
```

### Configuration Schema Details

#### Core Settings

| Field          | Type    | Default            | Description                                           |
| -------------- | ------- | ------------------ | ----------------------------------------------------- |
| `enabled`      | boolean | `true`             | Enable/disable caching globally                       |
| `default_mode` | string  | `"read-write"`     | Default cache mode for tasks without explicit setting |
| `cache_dir`    | string  | `"~/.cache/cuenv"` | Directory for cache storage                           |
| `max_size`     | integer | `10737418240`      | Maximum cache size in bytes (10GB)                    |

#### Environment Variable Filtering

| Field         | Type          | Default                                                 | Description                                      |
| ------------- | ------------- | ------------------------------------------------------- | ------------------------------------------------ |
| `env_include` | array<string> | `["PATH", "HOME", "USER", "SHELL", "LANG", "CUENV_*"]`  | Environment variables to include in cache keys   |
| `env_exclude` | array<string> | `["RANDOM", "TEMP", "TMP", "TERM", "SSH_*", "DISPLAY"]` | Environment variables to exclude from cache keys |

#### Cache Management

| Field                  | Type    | Default | Description                                        |
| ---------------------- | ------- | ------- | -------------------------------------------------- |
| `eviction_policy`      | string  | `"lru"` | Cache eviction policy (lru, lfu, fifo, size-based) |
| `stats_retention_days` | integer | `30`    | Number of days to retain cache statistics          |

#### Remote Cache Configuration

| Field              | Type    | Default | Description                                                    |
| ------------------ | ------- | ------- | -------------------------------------------------------------- |
| `endpoint`         | string  | `null`  | Remote cache server endpoint                                   |
| `auth_token`       | string  | `null`  | Authentication token (supports environment variable expansion) |
| `timeout_seconds`  | integer | `30`    | Timeout for remote operations                                  |
| `max_concurrent`   | integer | `10`    | Maximum concurrent remote requests                             |
| `upload_enabled`   | boolean | `true`  | Whether to upload results to remote cache                      |
| `download_enabled` | boolean | `true`  | Whether to download from remote cache                          |

#### Storage Configuration

| Field                     | Type    | Default | Description                                       |
| ------------------------- | ------- | ------- | ------------------------------------------------- |
| `inline_threshold`        | integer | `4096`  | Threshold for inline storage optimization (bytes) |
| `compression_enabled`     | boolean | `true`  | Enable compression of cached content              |
| `integrity_check_enabled` | boolean | `true`  | Enable integrity checking of cached content       |
| `gc_interval_seconds`     | integer | `300`   | Garbage collection interval in seconds            |

## Per-Task Cache Configuration

### CUE Schema

```cue
// Enhanced task configuration with cache settings
tasks: {
  [string]: {
    // Basic cache control
    cache?: bool
    cacheKey?: string

    // Advanced cache configuration
    cacheConfig?: {
      // Override global environment variable inclusion
      envInclude?: [...string]
      // Override global environment variable exclusion
      envExclude?: [...string]
      // Additional input patterns for this task
      extraInputs?: [...string]
      // Files/directories that should not affect cache key
      ignoreInputs?: [...string]
      // Custom cache key components
      customKeyComponents?: {
        [string]: string
      }
      // Cache-specific timeout
      timeout?: uint
      // Cache-specific size limits
      maxSize?: uint
    }

    // ... existing task configuration fields
  }
}
```

### Example Configurations

#### Basic Cache Control

```cue
tasks: {
  "build": {
    description: "Build the project"
    command: "make build"
    cache: true  // Enable caching (uses global defaults)
    inputs: ["src/**", "Makefile"]
    outputs: ["build/**"]
  }

  "deploy": {
    description: "Deploy to production"
    command: "./deploy.sh"
    cache: false  // Disable caching for deployments
  }
}
```

#### Advanced Cache Configuration

```cue
tasks: {
  "compile": {
    description: "Compile source code"
    command: "cargo build --release"
    cache: true
    inputs: ["src/**", "Cargo.toml", "Cargo.lock"]
    outputs: ["target/release/**"]
    cacheConfig: {
      // Only include compiler-related environment variables
      envInclude: ["PATH", "HOME", "RUST*", "CC", "CXX"]
      // Ignore temporary directories
      ignoreInputs: ["target/**", "*.tmp"]
      // Custom cache key components
      customKeyComponents: {
        rust_version: "1.70.0"
        target: "x86_64-unknown-linux-gnu"
      }
      // Larger cache size for compilation artifacts
      maxSize: 5368709120  // 5GB
    }
  }

  "test": {
    description: "Run tests"
    command: "cargo test"
    cache: true
    inputs: ["src/**", "tests/**", "Cargo.toml", "Cargo.lock"]
    cacheConfig: {
      // Exclude test-specific environment variables
      envExclude: ["TEST_*", "RUST_TEST_*"]
      // Additional test data inputs
      extraInputs: ["test_data/**"]
      // Custom timeout for test caching
      timeout: 1800  // 30 minutes
    }
  }
}
```

## Environment Variable Configuration

### Global Cache Control

| Environment Variable  | Values                               | Description                          |
| --------------------- | ------------------------------------ | ------------------------------------ |
| `CUENV_CACHE`         | `off`, `read`, `read-write`, `write` | Global cache mode override           |
| `CUENV_CACHE_DIR`     | path                                 | Override cache directory             |
| `CUENV_CACHE_SIZE`    | integer                              | Override maximum cache size in bytes |
| `CUENV_CACHE_ENABLED` | `true`, `false`                      | Enable/disable caching globally      |

### Remote Cache Configuration

| Environment Variable            | Values          | Description                           |
| ------------------------------- | --------------- | ------------------------------------- |
| `CUENV_REMOTE_CACHE_ENDPOINT`   | URL             | Remote cache server endpoint          |
| `CUENV_REMOTE_CACHE_AUTH_TOKEN` | string          | Authentication token for remote cache |
| `CUENV_REMOTE_CACHE_TIMEOUT`    | integer         | Remote cache timeout in seconds       |
| `CUENV_REMOTE_CACHE_UPLOAD`     | `true`, `false` | Enable uploading to remote cache      |
| `CUENV_REMOTE_CACHE_DOWNLOAD`   | `true`, `false` | Enable downloading from remote cache  |

### Debug and Monitoring

| Environment Variable  | Values          | Description                               |
| --------------------- | --------------- | ----------------------------------------- |
| `CUENV_CACHE_DEBUG`   | `true`, `false` | Enable debug logging for cache operations |
| `CUENV_CACHE_STATS`   | `true`, `false` | Enable detailed cache statistics          |
| `CUENV_CACHE_PROFILE` | `true`, `false` | Enable performance profiling              |

## Configuration Precedence

Configuration values are applied in the following order (highest priority first):

1. **Environment Variables** - Runtime overrides
2. **Per-Task Configuration** - Task-specific settings in CUE
3. **Project Configuration** - `.cuenv/cache.json`
4. **User Configuration** - `~/.config/cuenv/cache.json`
5. **Default Values** - Built-in defaults

### Example Configuration Resolution

```bash
# Environment variables
export CUENV_CACHE=read
export CUENV_CACHE_SIZE=5368709120  # 5GB

# User config: ~/.config/cuenv/cache.json
{
  "enabled": true,
  "default_mode": "read-write",
  "cache_dir": "~/.cache/cuenv",
  "max_size": 10737418240  # 10GB
}

# Project config: .cuenv/cache.json
{
  "enabled": true,
  "max_size": 2147483648  # 2GB
}

# Task config: env.cue
tasks: {
  "build": {
    cache: true,
    cacheConfig: {
      maxSize: 1073741824  # 1GB
    }
  }
}
```

**Resulting Configuration for "build" task:**

- `enabled`: `true` (from user config)
- `mode`: `read` (from `CUENV_CACHE` environment variable)
- `cache_dir`: `~/.cache/cuenv` (from user config)
- `max_size`: `5368709120` (from `CUENV_CACHE_SIZE` environment variable)
- `task_max_size`: `1073741824` (from task config)

## Configuration Validation

### Schema Validation Rules

1. **Path Validation**

   - `cache_dir` must be a valid absolute or relative path
   - Path expansion supports `~` for home directory
   - Path must be writable by the current user

2. **Size Validation**

   - `max_size` must be a positive integer
   - Minimum size: 1048576 bytes (1MB)
   - Maximum size: 1099511627776 bytes (1TB)

3. **Environment Variable Patterns**

   - `env_include` and `env_exclude` support glob patterns
   - Patterns must be valid regular expressions
   - Case-sensitive matching

4. **Remote Cache Validation**
   - `endpoint` must be a valid URL with grpc:// or grpcs:// scheme
   - `timeout_seconds` must be between 1 and 300
   - `max_concurrent` must be between 1 and 100

### Validation Error Messages

```json
{
	"errors": [
		{
			"field": "cache_dir",
			"message": "Cache directory '/invalid/path' does not exist or is not writable",
			"suggestion": "Create the directory or choose a different path"
		},
		{
			"field": "max_size",
			"message": "Cache size must be at least 1048576 bytes (1MB)",
			"suggestion": "Set max_size to a value >= 1048576"
		},
		{
			"field": "remote_cache.endpoint",
			"message": "Invalid URL scheme 'http://', expected 'grpc://' or 'grpcs://'",
			"suggestion": "Use grpc://cache.example.com:9092 format"
		}
	]
}
```

## Configuration Migration

### From Current Implementation

The current implementation uses a simple boolean `cache` field in task configuration. The new system maintains backward compatibility:

```cue
// Old configuration (still supported)
tasks: {
  "build": {
    cache: true
  }
}

// Automatically migrated to equivalent new configuration
tasks: {
  "build": {
    cache: true,
    cacheConfig: {}  // Uses global defaults
  }
}
```

### Migration Commands

```bash
# Validate current configuration
cuenv cache validate

# Show current configuration with resolution
cuenv cache config --show-resolution

# Generate default configuration file
cuenv cache init --config-file ~/.config/cuenv/cache.json

# Migrate from old cache format
cuenv cache migrate
```

## Configuration Examples

### Development Environment

```json
{
	"enabled": true,
	"default_mode": "read-write",
	"cache_dir": "~/.cache/cuenv",
	"max_size": 5368709120,
	"env_include": [
		"PATH",
		"HOME",
		"USER",
		"SHELL",
		"LANG",
		"CUENV_*",
		"RUST*",
		"NODE_*"
	],
	"env_exclude": ["RANDOM", "TEMP", "TMP", "TERM", "SSH_*"],
	"eviction_policy": "lru",
	"remote_cache": null
}
```

### CI/CD Environment

```json
{
	"enabled": true,
	"default_mode": "read",
	"cache_dir": "/tmp/cuenv-cache",
	"max_size": 10737418240,
	"env_include": ["PATH", "HOME", "USER", "CI", "GITHUB_*", "RUST*", "NODE_*"],
	"env_exclude": ["RANDOM", "TEMP", "TMP", "TERM", "SSH_*"],
	"eviction_policy": "fifo",
	"remote_cache": {
		"endpoint": "grpc://cache.company.com:9092",
		"auth_token": "${CI_CACHE_TOKEN}",
		"timeout_seconds": 60,
		"max_concurrent": 5,
		"upload_enabled": false,
		"download_enabled": true
	}
}
```

### Production Build Environment

```json
{
	"enabled": true,
	"default_mode": "read-write",
	"cache_dir": "/var/cache/cuenv",
	"max_size": 53687091200,
	"env_include": ["PATH", "HOME", "USER", "SHELL", "LANG", "CUENV_*"],
	"env_exclude": ["RANDOM", "TEMP", "TMP", "TERM", "SSH_*"],
	"eviction_policy": "lru",
	"remote_cache": {
		"endpoint": "grpcs://cache.company.com:9092",
		"auth_token": "${PROD_CACHE_TOKEN}",
		"timeout_seconds": 30,
		"max_concurrent": 20,
		"upload_enabled": true,
		"download_enabled": true
	},
	"storage": {
		"inline_threshold": 8192,
		"compression_enabled": true,
		"integrity_check_enabled": true,
		"gc_interval_seconds": 600
	}
}
```
