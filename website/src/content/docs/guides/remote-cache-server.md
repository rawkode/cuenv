---
title: Remote Cache Integration with Bazel
description: Using external cache servers with cuenv builds
---

cuenv supports integration with standard Bazel remote cache servers, allowing you to leverage existing caching infrastructure for your builds.

## Overview

While cuenv has its own sophisticated local caching system, you can also integrate with external remote cache servers that implement the Bazel Remote Execution API protocol. This enables:

- Shared caching across teams
- Distributed build caching
- Integration with existing CI/CD pipelines

## Recommended Cache Servers

For remote caching needs, we recommend using established Bazel cache solutions:

### 1. **Bazel Remote Cache** (bazel-remote)

The reference implementation from the Bazel team:

```bash
docker run -v /path/to/cache:/data \
  -p 9090:8080 -p 9092:9092 \
  buchgr/bazel-remote-cache
```

### 2. **BuildBuddy**

Enterprise-grade build acceleration platform:

```bash
# Self-hosted
docker run -p 1985:1985 -p 8080:8080 \
  gcr.io/flame-public/buildbuddy-app-onprem
```

### 3. **Buildbarn**

Highly scalable remote execution and caching:

```bash
# See https://github.com/buildbarn/bb-deployments
```

## Configuring cuenv with Remote Cache

To use a remote cache with cuenv builds:

1. **Set up your cache server** (see recommendations above)

2. **Configure your build tools** to use the remote cache:

   For Bazel:

   ```bash
   # .bazelrc
   build --remote_cache=grpc://your-cache-server:9092
   build --remote_instance_name=your-project
   ```

   For Buck2:

   ```ini
   # .buckconfig
   [build]
   remote_cache = grpc://your-cache-server:9092
   ```

3. **Use cuenv for environment management** while your build tool handles caching:

   ```bash
   # cuenv manages the environment
   cuenv load

   # Build tool uses remote cache
   bazel build //... --remote_cache=grpc://cache:9092
   ```

## Best Practices

1. **Local + Remote**: Combine cuenv's local caching with remote caching for best performance
2. **Network Proximity**: Place cache servers close to your build infrastructure
3. **Security**: Use TLS and authentication for production cache servers
4. **Monitoring**: Track cache hit rates and performance metrics

## Why External Cache Servers?

We recommend using dedicated cache server solutions because:

1. **Specialization**: Purpose-built for high-performance distributed caching
2. **Ecosystem**: Rich tooling and integrations already exist
3. **Maintenance**: Active development and community support
4. **Features**: Advanced capabilities like remote execution, metrics, and UI

## Integration Example

Here's a complete example using cuenv with Bazel and a remote cache:

```cue
// env.cue
package main

env: {
    default: {
        BUILD_CACHE: "grpc://cache.example.com:9092"
        BAZEL_REMOTE_CACHE: "\(BUILD_CACHE)"
    }
}
```

```bash
# Load environment with cuenv
cuenv load

# Build with remote cache (URL from environment)
bazel build //... \
  --remote_cache=$BAZEL_REMOTE_CACHE \
  --remote_upload_local_results=true
```

This approach gives you the best of both worlds: cuenv's powerful environment management with industry-standard remote caching solutions.
