---
title: Unified Cache Architecture
description: Architecture overview of cuenv's production-grade caching system with Google-scale reliability
---

# Unified Cache Architecture

## Overview

The unified cache system provides a single, production-grade caching solution that consolidates the functionality of the previous scattered implementations. It is designed for Google-scale reliability with proper error handling, clean async/sync boundaries, and zero-cost abstractions.

## Design Principles

1. **Single Responsibility**: One cache implementation that does everything well
2. **Production-Grade Error Handling**: Every error includes context and recovery strategies
3. **Clean Async/Sync Bridge**: No hacky workarounds, proper use of `tokio::task::spawn_blocking`
4. **Zero-Copy Where Possible**: Minimize allocations and copies
5. **Type Safety**: Leverage Rust's type system for compile-time guarantees

## Architecture Components

### Core Traits (`traits.rs`)

```rust
#[async_trait]
pub trait Cache: Send + Sync + Debug {
    async fn get<T>(&self, key: &str) -> Result<Option<T>>;
    async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>;
    // ... other methods
}
```

The `Cache` trait defines the fundamental operations all cache implementations must support. It uses async methods to efficiently support both local and remote caches.

### Unified Implementation (`unified.rs`)

The `UnifiedCache` combines:

- **In-memory hot cache**: DashMap for lock-free concurrent access
- **Persistent storage**: Content-addressed file storage with atomic writes
- **Automatic cleanup**: Background task for expired entry removal
- **Statistics tracking**: Atomic counters for performance monitoring

```rust
struct CacheInner {
    config: CacheConfig,
    base_dir: PathBuf,
    memory_cache: DashMap<String, Arc<InMemoryEntry>>,
    stats: CacheStats,
    io_semaphore: Semaphore,  // Limits concurrent I/O
}
```

### Error Handling (`errors.rs`)

Production-grade error types with recovery hints:

```rust
pub enum CacheError {
    Io { path, operation, source, recovery_hint },
    Serialization { key, operation, source, recovery_hint },
    // ... other variants
}

pub enum RecoveryHint {
    Retry { after: Duration },
    ClearAndRetry,
    IncreaseCapacity { suggested_bytes },
    // ... other hints
}
```

### Async/Sync Bridge (`bridge.rs`)

Clean bridging without hacks:

```rust
pub struct SyncCache {
    cache: Arc<UnifiedCache>,
    runtime: RuntimeHandle,
}

enum RuntimeHandle {
    Owned(Runtime),      // We own the runtime
    Borrowed(Handle),    // Using existing runtime
}
```

## Data Flow

### Write Path

1. **Validation**: Key validation and capacity check
2. **Serialization**: Value serialized with bincode
3. **Hashing**: Content hash computed for integrity
4. **Memory Storage**: Store in DashMap for hot access
5. **Disk Storage**: Atomic write to content-addressed path
6. **Statistics Update**: Atomic counter updates

### Read Path

1. **Memory Check**: Fast path via DashMap
2. **Expiration Check**: Validate TTL if set
3. **Disk Fallback**: Load from disk if not in memory
4. **Memory Population**: Cache hot data in memory
5. **Statistics Update**: Track hits/misses

## Key Design Decisions

### Why DashMap?

- Lock-free concurrent access
- Better performance than `Arc<RwLock<HashMap>>`
- No global locks for read operations
- Automatic sharding for reduced contention

### Why Content-Addressed Storage?

- Automatic deduplication
- Integrity verification built-in
- Efficient for large values
- Natural cache key distribution

### Why Semaphore for I/O?

- Prevents file descriptor exhaustion
- Controlled parallelism for disk operations
- Backpressure mechanism
- Configurable concurrency limit

### Why Separate Async and Sync?

- Clean API for both contexts
- No runtime creation in async contexts
- Proper use of `spawn_blocking`
- Type-safe context detection

## Performance Characteristics

### Memory Usage

- Hot cache limited by configuration
- Metadata overhead: ~200 bytes per entry
- Automatic eviction of expired entries
- Zero-copy for large values via Arc

### Latency

- Memory hits: < 1μs
- Disk reads: < 1ms (SSD)
- Network reads: 10-100ms (Phase 2)
- Batch operations: Parallelized

### Throughput

- Concurrent reads: No limit
- Concurrent writes: Limited by I/O semaphore
- Batch operations: Up to 100x single ops
- Background cleanup: Non-blocking

## Future Enhancements (Phase 2)

1. **Remote Cache Support**
   - gRPC protocol for efficiency
   - Consistent hashing for distribution
   - Read-through/write-through patterns

2. **Advanced Features**
   - Compression for values > threshold
   - Encryption for sensitive data
   - Multi-tier caching (L1/L2/L3)
   - Cache warming strategies

3. **Observability**
   - OpenTelemetry integration
   - Prometheus metrics
   - Distributed tracing
   - Performance profiling hooks

## Usage Patterns

### Basic Usage

```rust
// Async context
let cache = CacheBuilder::new("/path/to/cache")
    .with_config(config)
    .build_async()
    .await?;

cache.put("key", &value, Some(Duration::from_secs(300))).await?;
let value: Option<MyType> = cache.get("key").await?;
```

### Error Recovery

```rust
match cache.get::<Value>(key).await {
    Ok(Some(value)) => // Use value,
    Ok(None) => // Key not found,
    Err(e) => match e.recovery_hint() {
        RecoveryHint::Retry { after } => {
            tokio::time::sleep(*after).await;
            // Retry
        }
        RecoveryHint::ClearAndRetry => {
            cache.clear().await?;
            // Retry
        }
        _ => return Err(e),
    }
}
```

### Production Monitoring

```rust
let stats = cache.statistics().await?;
metrics::gauge!("cache.hit_rate",
    stats.hits as f64 / (stats.hits + stats.misses) as f64);
metrics::gauge!("cache.size_bytes", stats.total_bytes as f64);
metrics::counter!("cache.errors", stats.errors);
```

## Testing Strategy

1. **Unit Tests**: Each component tested in isolation
2. **Integration Tests**: Full cache workflow testing
3. **Concurrency Tests**: Race condition detection
4. **Performance Tests**: Benchmark critical paths
5. **Chaos Tests**: Failure injection and recovery

## Security Considerations

1. **Path Traversal**: Validated key format prevents directory escapes
2. **Resource Limits**: Configurable capacity prevents DoS
3. **Atomic Operations**: Prevents partial writes/corruption
4. **Future**: Encryption for sensitive cached data
