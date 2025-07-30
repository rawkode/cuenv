# Cache System Migration Guide

This guide helps you migrate from the legacy cache implementations to the new unified cache system.

## Overview

The new cache system consolidates multiple overlapping implementations into a single, production-grade `UnifiedCache` with proper error handling and clean async/sync boundaries.

### What's Changed

1. **Single Cache Interface**: Replace `ActionCache`, `CacheManager`, `ConcurrentCache`, and `ContentAddressedStore` with `UnifiedCache`
2. **Production Error Handling**: New `CacheError` type with recovery hints instead of unwrap()
3. **Clean Async/Sync Bridge**: Use `SyncCache` wrapper instead of hacky `run_async()`
4. **Unified Configuration**: Single `CacheConfiguration` type

## Migration Steps

### 1. Update Imports

```rust
// Old
use cuenv::cache::{CacheManager, ActionCache, ConcurrentCache};

// New
use cuenv::cache::{CacheBuilder, UnifiedCache, SyncCache};
```

### 2. Create Cache Instances

```rust
// Old - Multiple cache types
let engine = CacheEngine::new()?;
let cas = ContentAddressedStore::new(path, threshold)?;
let action_cache = ActionCache::new(cas, max_size, &cache_dir)?;
let cache_manager = CacheManager::new_sync()?;

// New - Single unified cache
// For async contexts:
let cache = CacheBuilder::new("/path/to/cache")
    .with_config(CacheConfiguration::default())
    .build_async()
    .await?;

// For sync contexts:
let cache = CacheBuilder::new("/path/to/cache")
    .build_sync()?;
```

### 3. Update Cache Operations

```rust
// Old - CacheManager
let result = run_async(cache_manager.get_task_result(key))?;
run_async(cache_manager.store_task_result(key, &result))?;

// New - Async
let result: Option<TaskResult> = cache.get(key).await?;
cache.put(key, &result, Some(Duration::from_secs(3600))).await?;

// New - Sync
let result: Option<TaskResult> = sync_cache.get(key)?;
sync_cache.put(key, &result, Some(Duration::from_secs(3600)))?;
```

### 4. Handle Errors Properly

```rust
// Old
let value = cache.get(key).unwrap();

// New
match cache.get::<Value>(key).await {
    Ok(Some(value)) => {
        // Use value
    }
    Ok(None) => {
        // Key not found
    }
    Err(e) => {
        // Handle error with recovery hint
        match e.recovery_hint() {
            RecoveryHint::Retry { after } => {
                tokio::time::sleep(*after).await;
                // Retry operation
            }
            RecoveryHint::ClearAndRetry => {
                cache.clear().await?;
                // Retry operation
            }
            _ => return Err(e),
        }
    }
}
```

### 5. Update Task Executor

```rust
// Old
let cache_manager = CacheManager::new_sync()?;
if let Some(result) = run_async(cache_manager.get_task_result(&cache_key))? {
    return Ok(result);
}

// New
let cache = CacheBuilder::new(cache_dir).build_sync()?;
if let Some(result) = cache.get::<CachedTaskResult>(&cache_key)? {
    return Ok(result);
}
```

## Feature Mapping

| Old Feature                        | New Implementation                         |
| ---------------------------------- | ------------------------------------------ |
| `ActionCache::compute_digest()`    | Use `CacheKeyGenerator` separately         |
| `CacheManager::verify_integrity()` | Built into `UnifiedCache::get()`           |
| `ContentAddressedStore::store()`   | `UnifiedCache::put()` with content hashing |
| `ConcurrentCache` stats            | `UnifiedCache::statistics()`               |
| Remote cache support               | Coming in Phase 2                          |

## Best Practices

1. **Use Type Parameters**: Always specify the type when calling `get()`:

   ```rust
   let value: Option<MyType> = cache.get(key).await?;
   ```

2. **Handle All Errors**: Never use unwrap() or expect():

   ```rust
   let value = cache.get::<Value>(key).await
       .map_err(|e| {
           tracing::error!("Cache error: {}", e);
           e
       })?;
   ```

3. **Use Recovery Hints**: Follow the suggested recovery strategy:

   ```rust
   if e.is_transient() {
       // Retry transient errors
   } else if e.is_corruption() {
       // Clear corrupted data
   }
   ```

4. **Prefer Async**: Use async cache when possible for better performance:

   ```rust
   // In async context
   let cache = CacheBuilder::new(path).build_async().await?;

   // Only use sync when necessary
   let sync_cache = CacheBuilder::new(path).build_sync()?;
   ```

## Deprecation Timeline

- **Phase 1** (Current): New unified cache available, old implementations deprecated
- **Phase 2**: Remote cache support added to unified implementation
- **Phase 3**: Old implementations removed completely

Update your code now to avoid breaking changes in future releases.
