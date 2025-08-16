---
title: Cache System
description: Basic caching system for task execution results
---

# Cache System

The cuenv cache system provides basic caching for task execution results to improve performance. It stores task outputs and metadata to avoid re-executing unchanged tasks.

## Architecture Overview

The cache system provides:

- **Local Storage**: File-based cache storage in the user's cache directory
- **Task Result Caching**: Automatic caching of task outputs based on input changes
- **Statistics Tracking**: Basic hit/miss rate and storage usage metrics

## Configuration

### Global Configuration

Cache can be configured via JSON configuration file at `~/.config/cuenv/config.json`:

```json
{
	"cache": {
		"enabled": true,
		"mode": "read-write",
		"max_size": 10737418240,
		"base_dir": "/path/to/cache"
	}
}
```

### Environment Variables

- `CUENV_CACHE` - Cache mode: "off", "read", "write", "read-write"
- `CUENV_CACHE_ENABLED` - Enable/disable cache: "true" or "false"
- `CUENV_CACHE_MAX_SIZE` - Maximum cache size in bytes
- `CUENV_CACHE_BASE_DIR` - Custom cache directory

## Task Caching

Tasks can be individually configured for caching in your env.cue file:

```cue
tasks: {
    build: {
        command: "cargo build --release"
        cache: true  // Simple enable/disable
    }

    test: {
        command: "cargo test"
        cache: {
            enabled: false  // Advanced configuration
            env: {
                include: ["CARGO_*"]
                exclude: ["RANDOM_SEED"]
            }
        }
    }
}
```

## Cache Storage

The cache stores task outputs and metadata in the user's cache directory (typically `~/.cache/cuenv/`). Task results are cached based on:

- Task command and arguments
- Input file contents and timestamps
- Environment variables (filtered)
- Working directory

## Maintenance

### Available Commands

```bash
# Clear all cache entries
cuenv cache clear

# Show cache statistics
cuenv cache stats

# Clean up stale cache entries (note: max-age-hours parameter currently ignored)
cuenv cache cleanup
```

### Cache Statistics

The `cuenv cache stats` command shows:

- **Hits**: Number of cache hits
- **Misses**: Number of cache misses
- **Writes**: Number of entries written
- **Errors**: Number of cache errors
- **Hit rate**: Percentage of requests that were cache hits
- **Total bytes saved**: Storage space saved by compression
