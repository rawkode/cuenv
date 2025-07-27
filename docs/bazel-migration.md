# Bazel Build Cache Integration for cuenv

This document describes the integration of Bazel as the build system for cuenv, with a focus on leveraging the existing cache infrastructure.

## Architecture Overview

The integration preserves cuenv's existing cache architecture while adding Bazel build capabilities:

```
┌─────────────────┐     ┌──────────────────┐
│   Bazel Build   │────▶│ Remote Cache API │
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
   nix develop -c "cargo run --bin remote_cache_server -- --address 127.0.0.1:50051"
   ```

2. **Build with Bazel:**
   ```bash
   nix develop -c "bazel build //... --config=remote"
   ```

3. **Run tests:**
   ```bash
   nix develop -c "bazel test //... --config=remote"
   ```

## Configuration

### Bazel Configuration (.bazelrc)

The `.bazelrc` file configures Bazel to use cuenv's remote cache:

```bash
# Enable remote caching
build:remote --remote_cache=grpc://localhost:50051
build:remote --remote_instance_name=cuenv-main
build:remote --remote_cache_compression
```

### Remote Cache Server Options

```bash
cuenv-remote-cache-server \
  --address 0.0.0.0:50051 \          # Listen address
  --cache-dir /var/cache/cuenv \     # Cache storage directory
  --max-cache-size 10737418240 \     # 10GB cache limit
  --enable-action-cache \            # Enable action result caching
  --enable-cas                       # Enable content-addressed storage
```

## Build Targets

### Main Binary
```bash
bazel build //:cuenv
```

### Library
```bash
bazel build //:cuenv_lib
```

### Tests
```bash
# All tests
bazel test //...

# Specific test
bazel test //:integration_test
```

### Benchmarks
```bash
bazel test //... --test_tag_filters=benchmark
```

## Platform-Specific Builds

### Linux
```bash
bazel build //:cuenv --config=linux
```

### macOS
```bash
bazel build //:cuenv --config=macos
```

## Integration with Existing Tools

### Using with Cargo

During the transition period, both build systems can coexist:

```bash
# Cargo build (existing)
cargo build --release

# Bazel build (new)
bazel build //:cuenv -c opt
```

### CI/CD Integration

```yaml
# GitHub Actions example
- name: Start Remote Cache
  run: |
    cargo run --bin remote_cache_server -- \
      --address 127.0.0.1:50051 &
    
- name: Build with Bazel
  run: bazel build //... --config=remote

- name: Test with Bazel
  run: bazel test //... --config=remote
```

## Performance Optimization

### Local Development
- Use `--config=debug` for faster builds with debug symbols
- Enable persistent workers: `--worker_sandboxing`

### CI Builds
- Use `--config=release` for optimized builds
- Enable remote caching: `--config=remote`
- Share cache across builds: `--remote_upload_local_results=true`

## Troubleshooting

### Cache Misses
Check cache server logs:
```bash
RUST_LOG=debug cargo run --bin remote_cache_server
```

### Build Failures
Enable verbose logging:
```bash
bazel build //:cuenv --sandbox_debug --verbose_failures
```

### Performance Issues
Profile the build:
```bash
bazel build //:cuenv --profile=/tmp/profile.gz
bazel analyze-profile /tmp/profile.gz
```

## Future: Buck2 Migration Path

The architecture supports future migration to Buck2:

1. Both Bazel and Buck2 use the same Remote Execution API
2. The remote cache server works with both systems
3. Migration involves changing build files, not cache infrastructure

When ready to migrate:
```bash
# Same cache server, different build tool
buck2 build //:cuenv --remote-cache grpc://localhost:50051
```

## Conclusion

This integration provides:
- ✅ Improved build performance through caching
- ✅ Distributed builds capability
- ✅ Compatibility with existing infrastructure
- ✅ Future-proof architecture supporting Buck2
- ✅ Minimal disruption to current workflow