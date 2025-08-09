//! Clean async/sync bridge for cache operations
//!
//! This module provides production-grade bridging between async and sync contexts
//! using tokio::task::spawn_blocking and proper runtime management.

use crate::core::Cache;
use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::{Cache as CacheTrait, CacheMetadata, CacheStatistics};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Handle, Runtime};

/// Sync wrapper for the async cache
///
/// This provides a synchronous interface to the async Cache
/// for use in non-async contexts.
pub struct SyncCache {
    /// The underlying async cache
    cache: Arc<Cache>,
    /// Runtime handle for executing async operations
    runtime: RuntimeHandle,
}

/// Handle to a Tokio runtime
enum RuntimeHandle {
    /// We own the runtime
    Owned(Runtime),
    /// We're using an existing runtime
    Borrowed(Handle),
}

impl SyncCache {
    /// Create a new sync cache with its own runtime
    pub fn new(cache: Cache) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CacheError::Configuration {
                message: format!("Failed to create runtime: {e}"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check system resources".to_string(),
                },
            })?;

        Ok(Self {
            cache: Arc::new(cache),
            runtime: RuntimeHandle::Owned(runtime),
        })
    }

    /// Create a sync cache using the current async runtime
    ///
    /// This should be called from within an async context.
    pub fn from_async(cache: Cache) -> Result<Self> {
        let handle = Handle::try_current().map_err(|_| CacheError::Configuration {
            message: "No async runtime found".to_string(),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Call from within an async context or use new()".to_string(),
            },
        })?;

        Ok(Self {
            cache: Arc::new(cache),
            runtime: RuntimeHandle::Borrowed(handle),
        })
    }

    /// Execute an async operation in the sync context
    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        match &self.runtime {
            RuntimeHandle::Owned(runtime) => runtime.block_on(future),
            RuntimeHandle::Borrowed(handle) => {
                // We're in an async context, use spawn_blocking
                handle.block_on(async move {
                    tokio::task::spawn_blocking(move || Handle::current().block_on(future))
                        .await
                        .map_err(|e| CacheError::Configuration {
                            message: format!("Task panicked: {e}"),
                            recovery_hint: RecoveryHint::Manual {
                                instructions: "Check for panics in cache operations".to_string(),
                            },
                        })?
                })
            }
        }
    }

    /// Get a value from the cache
    pub fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let cache: Arc<Cache> = Arc::clone(&self.cache);
        let key = key.to_string();

        self.block_on(async move { cache.get(&key).await })
    }

    /// Store a value in the cache
    pub fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync + Clone + 'static,
    {
        let cache: Arc<Cache> = Arc::clone(&self.cache);
        let key = key.to_string();
        let value = value.clone();

        self.block_on(async move { cache.put(&key, &value, ttl).await })
    }

    /// Remove a value from the cache
    pub fn remove(&self, key: &str) -> Result<bool> {
        let cache: Arc<Cache> = Arc::clone(&self.cache);
        let key = key.to_string();

        self.block_on(async move { cache.remove(&key).await })
    }

    /// Check if a key exists in the cache
    pub fn contains(&self, key: &str) -> Result<bool> {
        let cache: Arc<Cache> = Arc::clone(&self.cache);
        let key = key.to_string();

        self.block_on(async move { cache.contains(&key).await })
    }

    /// Get metadata about a cached entry
    pub fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>> {
        let cache: Arc<Cache> = Arc::clone(&self.cache);
        let key = key.to_string();

        self.block_on(async move { cache.metadata(&key).await })
    }

    /// Clear all entries from the cache
    pub fn clear(&self) -> Result<()> {
        let cache: Arc<Cache> = Arc::clone(&self.cache);

        self.block_on(async move { cache.clear().await })
    }

    /// Get cache statistics
    pub fn statistics(&self) -> Result<CacheStatistics> {
        let cache: Arc<Cache> = Arc::clone(&self.cache);

        self.block_on(async move { cache.statistics().await })
    }
}

impl fmt::Debug for SyncCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncCache")
            .field("cache", &self.cache)
            .finish()
    }
}

/// Builder for creating caches with automatic sync/async detection
pub struct CacheBuilder {
    config: crate::traits::CacheConfig,
    base_dir: std::path::PathBuf,
}

impl CacheBuilder {
    /// Create a new cache builder
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            config: Default::default(),
            base_dir: base_dir.into(),
        }
    }

    /// Set the cache configuration
    pub fn with_config(mut self, config: crate::traits::CacheConfig) -> Self {
        self.config = config;
        self
    }

    /// Build an async cache
    pub async fn build_async(self) -> Result<Cache> {
        Cache::new(self.base_dir, self.config).await
    }

    /// Build a sync cache
    pub fn build_sync(self) -> Result<SyncCache> {
        // Check if we're in an async context
        if Handle::try_current().is_ok() {
            // We're in async context, use spawn_blocking
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| CacheError::Configuration {
                        message: format!("Failed to create runtime: {e}"),
                        recovery_hint: RecoveryHint::Manual {
                            instructions: "Check system resources".to_string(),
                        },
                    })?;

                let cache = runtime.block_on(Cache::new(self.base_dir, self.config))?;
                SyncCache::new(cache)
            })
            .join()
            .map_err(|_| CacheError::Configuration {
                message: "Thread panicked while creating cache".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check for panics during cache initialization".to_string(),
                },
            })?
        } else {
            // Not in async context, create directly
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CacheError::Configuration {
                    message: format!("Failed to create runtime: {e}"),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check system resources".to_string(),
                    },
                })?;

            let cache = runtime.block_on(Cache::new(self.base_dir, self.config))?;
            SyncCache::new(cache)
        }
    }

    /// Build a cache automatically based on the current context
    ///
    /// Returns the concrete cache type for maximum flexibility.
    pub async fn build_auto(self) -> Result<Cache> {
        Cache::new(self.base_dir, self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_cache_operations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheBuilder::new(temp_dir.path()).build_sync()?;

        // Test basic operations
        cache.put("key1", &"value1", None)?;
        let value: Option<String> = cache.get("key1")?;
        assert_eq!(value, Some("value1".to_string()));

        assert!(cache.contains("key1")?);
        assert!(cache.remove("key1")?);
        assert!(!cache.contains("key1")?);

        Ok(())
    }

    #[tokio::test]
    async fn test_async_cache_operations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheBuilder::new(temp_dir.path()).build_async().await?;

        // Test basic operations
        cache.put("key1", &"value1", None).await?;
        let value: Option<String> = cache.get("key1").await?;
        assert_eq!(value, Some("value1".to_string()));

        assert!(cache.contains("key1").await?);
        assert!(cache.remove("key1").await?);
        assert!(!cache.contains("key1").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_cache_from_async_context() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let async_cache = CacheBuilder::new(temp_dir.path()).build_async().await?;
        let sync_cache = SyncCache::from_async(async_cache)?;

        // Test that sync operations work from a blocking context spawned from async
        let result = tokio::task::spawn_blocking(move || -> Result<()> {
            // Test that sync operations work
            sync_cache.put("key1", &42, None)?;
            let value: Option<i32> = sync_cache.get("key1")?;
            assert_eq!(value, Some(42));
            Ok(())
        })
        .await;

        match result {
            Ok(inner_result) => inner_result,
            Err(join_error) => panic!("Task panicked: {}", join_error),
        }
    }
}
