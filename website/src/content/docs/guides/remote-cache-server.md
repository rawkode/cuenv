---
title: Remote Cache Server
description: Use cuenv as a remote cache backend for Bazel/Buck2 builds
---

cuenv provides a remote cache server that implements the Bazel/Buck2 Remote Execution API, allowing you to use cuenv's sophisticated caching infrastructure as a backend for your Bazel or Buck2 builds.

## Overview

The remote cache server exposes cuenv's existing cache infrastructure via the standard Remote Execution API protocol:

```
┌─────────────────┐     ┌──────────────────┐
│ Bazel/Buck2     │────▶│ cuenv Remote     │
│ Build System    │     │ Cache Server     │
└─────────────────┘     └──────────────────┘
                               │
                               ▼
                    ┌──────────────────────┐
                    │   cuenv Cache Layer  │
                    ├──────────────────────┤
                    │ • ContentAddressedStore
                    │ • ActionCache        │
                    │ • ConcurrentCache    │
                    └──────────────────────┘
```

## Quick Start

1. **Start the remote cache server:**
   ```bash
   cuenv remote-cache-server --address 0.0.0.0:50051
   ```

2. **Configure Bazel to use cuenv's cache:**
   ```bash
   bazel build //... --remote_cache=grpc://localhost:50051
   ```

3. **Or configure Buck2:**
   ```bash
   buck2 build //... --remote-cache grpc://localhost:50051
   ```

## Server Options

```bash
cuenv remote-cache-server \
  --address 0.0.0.0:50051 \          # Listen address
  --cache-dir /var/cache/cuenv \     # Cache storage directory
  --max-cache-size 10737418240       # 10GB cache limit
```

## Using with Bazel

Add to your `.bazelrc`:

```bash
# Use cuenv as remote cache
build --remote_cache=grpc://cache.example.com:50051
build --remote_instance_name=my-project
build --remote_timeout=3600

# Enable compression
build --remote_cache_compression

# Upload local results to share with team
build --remote_upload_local_results=true
```

## Using with Buck2

Configure in `.buckconfig`:

```ini
[build]
remote_cache = grpc://cache.example.com:50051
remote_instance_name = my-project
```

## Features

- **Content-Addressed Storage**: Efficient deduplication using SHA256 hashes
- **Action Cache**: Cache build action results for incremental builds
- **Compression**: Automatic compression for network efficiency
- **Concurrent Access**: Lock-free design for high-performance parallel builds
- **Platform Support**: Works on Linux, macOS, and Windows

## Deployment

### Docker

```dockerfile
FROM rust:1.79-slim
WORKDIR /app
COPY . .
RUN cargo build --release --bin remote_cache_server
EXPOSE 50051
CMD ["./target/release/remote_cache_server", "--address", "0.0.0.0:50051"]
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cuenv-cache
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
        image: cuenv/remote-cache:latest
        ports:
        - containerPort: 50051
        env:
        - name: CACHE_DIR
          value: /data/cache
        volumeMounts:
        - name: cache-storage
          mountPath: /data/cache
      volumes:
      - name: cache-storage
        persistentVolumeClaim:
          claimName: cuenv-cache-pvc
```

## Monitoring

The server exposes metrics at `/metrics`:

- `cache_hits_total` - Number of cache hits
- `cache_misses_total` - Number of cache misses
- `cache_size_bytes` - Current cache size
- `cache_evictions_total` - Number of evicted entries

## Security

- TLS support via `--tls-cert` and `--tls-key` flags
- Authentication via `--auth-token` flag
- Rate limiting to prevent abuse

## Comparison with Other Cache Servers

| Feature | cuenv | BuildBuddy | Buildbarn | bazel-remote |
|---------|-------|------------|-----------|--------------|
| Protocol | RE API | RE API | RE API | RE API |
| Language | Rust | Go | Go | Go |
| CAS | ✓ | ✓ | ✓ | ✓ |
| Action Cache | ✓ | ✓ | ✓ | ✓ |
| Inline Storage | ✓ | ✗ | ✗ | ✗ |
| Lock-free | ✓ | ✗ | ✗ | ✗ |
| Platform-native | ✓ | ✗ | ✗ | ✗ |

## Why Use cuenv's Remote Cache?

1. **Performance**: Written in Rust with lock-free concurrent access
2. **Efficiency**: Inline storage optimization for small objects
3. **Integration**: Seamlessly integrates with cuenv's build features
4. **Flexibility**: Works with any RE API-compatible build system
5. **Production-Ready**: Battle-tested caching infrastructure