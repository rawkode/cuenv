---
title: Remote Cache Configuration
description: Configure and deploy cuenv's Bazel-compatible remote cache server for distributed builds
---

# Remote Cache Configuration Guide

This guide explains how to configure and use cuenv's Bazel-compatible remote cache server and client.

## Overview

cuenv provides a fully-compliant Bazel Remote Execution API v2 cache server that can be used with:

- Bazel
- Buck2
- Any other build system supporting the Remote Execution API

## Server Setup

### Starting the Bazel Cache Server

```bash
# Start with default settings
bazel_cache_server

# Custom configuration
bazel_cache_server \
  --address 0.0.0.0:50051 \
  --cache-dir /var/cache/cuenv \
  --max-cache-size 107374182400 \  # 100GB
  --max-batch-size 1000 \
  --max-blob-size 1073741824        # 1GB
```

### Server Command-Line Options

| Option                           | Default              | Description                                 |
| -------------------------------- | -------------------- | ------------------------------------------- |
| `--address`                      | `127.0.0.1:50051`    | Server bind address                         |
| `--cache-dir`                    | `/var/cache/cuenv`   | Base directory for cache storage            |
| `--max-cache-size`               | `10737418240` (10GB) | Maximum cache size in bytes                 |
| `--max-batch-size`               | `1000`               | Maximum number of blobs per batch operation |
| `--max-blob-size`                | `1073741824` (1GB)   | Maximum size of individual blobs            |
| `--enable-action-cache`          | `true`               | Enable action cache service                 |
| `--enable-cas`                   | `true`               | Enable content-addressed storage            |
| `--circuit-breaker-threshold`    | `0.5`                | Failure rate threshold (0.0-1.0)            |
| `--circuit-breaker-timeout-secs` | `60`                 | Circuit breaker recovery timeout            |
| `--inline-threshold`             | `1024` (1KB)         | Size threshold for inline storage           |
| `--log-level`                    | `info`               | Log level (trace, debug, info, warn, error) |

## Client Configuration

### Bazel Configuration

Add to your `.bazelrc`:

```bash
# Basic remote cache configuration
build --remote_cache=grpc://localhost:50051

# With authentication (when implemented)
build --remote_cache=grpc://cache.example.com:50051
build --remote_cache_header=x-api-key=your-api-key

# Performance tuning
build --remote_max_connections=10
build --remote_timeout=30
build --remote_retries=3
```

### Buck2 Configuration

Add to your `.buckconfig`:

```ini
[buck2]
remote_cache = grpc://localhost:50051

[remote]
max_concurrent_uploads = 10
max_concurrent_downloads = 10
timeout_seconds = 30
```

### cuenv Configuration

Configure remote caching in your `env.cue`:

```cue
// Global remote cache configuration
remoteCache: {
    // Server address
    server: "grpc://cache.example.com:50051"

    // Connection settings
    connectTimeout: "10s"
    requestTimeout: "30s"

    // Retry configuration
    maxRetries: 3
    retryBackoff: "100ms"

    // Circuit breaker settings
    circuitBreakerThreshold: 0.5
    circuitBreakerTimeout: "60s"

    // Cache behavior
    uploadOnMiss: true      // Upload local cache misses to remote
    populateLocal: true     // Populate local cache from remote hits
    failOnRemoteError: false // Continue if remote is unavailable
}

// Per-task remote cache settings
tasks: {
    build: {
        cache: true
        cacheConfig: {
            // Override global settings for this task
            remoteCache: {
                server: "grpc://fast-cache.example.com:50051"
                requestTimeout: "10s"
            }
        }
    }
}
```

## Deployment

### Docker Deployment

```dockerfile
FROM rust:latest as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin bazel_cache_server

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bazel_cache_server /usr/local/bin/

EXPOSE 50051

CMD ["bazel_cache_server", "--address", "0.0.0.0:50051"]
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cuenv-cache-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: cuenv-cache
  template:
    metadata:
      labels:
        app: cuenv-cache
    spec:
      containers:
        - name: cache-server
          image: cuenv/bazel-cache-server:latest
          ports:
            - containerPort: 50051
          env:
            - name: CACHE_DIR
              value: /cache
            - name: MAX_CACHE_SIZE
              value: "107374182400" # 100GB
          volumeMounts:
            - name: cache-storage
              mountPath: /cache
      volumes:
        - name: cache-storage
          persistentVolumeClaim:
            claimName: cache-pvc

---
apiVersion: v1
kind: Service
metadata:
  name: cuenv-cache
spec:
  selector:
    app: cuenv-cache
  ports:
    - port: 50051
      targetPort: 50051
  type: LoadBalancer
```

## Performance Tuning

### Server Performance

1. **Storage Backend**: Use SSD storage for best performance
2. **Memory**: Allocate sufficient memory for index caching
3. **Network**: Use low-latency network connections
4. **Compression**: Enable compression for large blobs

### Client Performance

1. **Batch Operations**: The client automatically batches operations up to `max_batch_size`
2. **Circuit Breaker**: Prevents cascading failures when remote cache is unavailable
3. **Local Fallback**: Continues with local cache if remote is unavailable

## Monitoring

### Metrics

The server exposes the following metrics:

- `cas.find_missing_blobs` - Number of FindMissingBlobs requests
- `cas.batch_update_blobs` - Number of BatchUpdateBlobs requests
- `cas.batch_read_blobs` - Number of BatchReadBlobs requests
- `action_cache.get` - Number of GetActionResult requests
- `action_cache.update` - Number of UpdateActionResult requests

### Health Checks

The server provides gRPC health checks:

```bash
# Check server health
grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check
```

## Security

### TLS Configuration

For production deployments, use TLS:

```bash
# Server with TLS
bazel_cache_server \
  --tls-cert /path/to/cert.pem \
  --tls-key /path/to/key.pem

# Client configuration
build --remote_cache=grpcs://cache.example.com:50051
```

### Authentication (Future)

Authentication support is planned for future releases:

```bash
# API key authentication
build --remote_cache_header=x-api-key=your-api-key

# JWT authentication
build --remote_cache_header=authorization=Bearer your-jwt-token
```

## Troubleshooting

### Common Issues

1. **Connection Refused**

   - Check server is running: `ps aux | grep bazel_cache_server`
   - Verify port is open: `nc -zv localhost 50051`

2. **Circuit Breaker Open**

   - Check server logs for errors
   - Verify network connectivity
   - Wait for circuit breaker timeout

3. **Cache Misses**
   - Verify hash compatibility (SHA256)
   - Check blob size limits
   - Ensure proper instance name

### Debug Logging

Enable debug logging for troubleshooting:

```bash
# Server
bazel_cache_server --log-level debug

# Bazel client
build --remote_cache=grpc://localhost:50051 --remote_cache_debug
```

## Compatibility

### Supported Features

- ✅ Content-Addressed Storage (CAS)
- ✅ Action Cache
- ✅ Batch operations
- ✅ SHA256 digest function
- ✅ Circuit breaker fault tolerance
- ✅ gRPC reflection

### Limitations

- ❌ Remote execution (cache only)
- ❌ SHA1/MD5 digests (SHA256 only)
- ❌ Symlink support (planned)

## Migration from Other Cache Servers

### From Bazel Remote

```bash
# Export data from bazel-remote
bazel-remote --export-dir /tmp/export

# Import to cuenv
cuenv-cache-import --input /tmp/export --cache-dir /var/cache/cuenv
```

### From BuildBuddy

The cache format is compatible. Simply point your Bazel configuration to the cuenv server:

```bash
# Before
build --remote_cache=grpc://buildbuddy.io:443

# After
build --remote_cache=grpc://your-cuenv-server:50051
```

## Best Practices

1. **High Availability**: Deploy multiple cache servers behind a load balancer
2. **Backup**: Regular backups of the cache directory
3. **Monitoring**: Set up alerts for cache hit rate and errors
4. **Capacity Planning**: Monitor disk usage and plan for growth
5. **Network Optimization**: Deploy cache servers close to build machines

## Future Enhancements

Planned features for future releases:

- Authentication and authorization
- Metrics export (Prometheus)
- Cache replication
- S3/GCS backend support
- Web UI for cache management
