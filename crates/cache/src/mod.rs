//! High-performance caching system for cuenv tasks
//!
//! This module provides a unified, production-grade caching infrastructure
//! with clean architecture and proper async/sync boundaries.

// Core modules
mod bridge;
mod config;
mod core;
mod engine;
mod errors;
mod eviction;
#[path = "fast-path/mod.rs"]
mod fast_path;
mod hashing;
mod item;
mod keys;
mod manager;
mod memory_manager;
mod mode;
mod monitored;
mod monitoring;
mod performance;
mod serialization;
mod storage;
mod streaming;
mod traits;
mod types;
#[path = "warming/mod.rs"]
mod warming;

// Feature modules
mod concurrent;
mod metrics;
mod security;

// Legacy implementations (deprecated - will be removed)
mod content_addressed_store;

// Public exports - new unified API
pub use bridge::{CacheBuilder, SyncCache};
pub use core::Cache as ProductionCache;
pub use errors::{CacheError, RecoveryHint, Result as CacheResult};
pub use metrics::endpoint::MetricsEndpoint;
pub use monitored::{MonitoredCache, MonitoredCacheBuilder};
pub use monitoring::{CacheMonitor, HitRateReport, RealTimeStatsReport, TracedOperation};
pub use storage::{CompressionConfig, StorageBackend};
pub use streaming::{CacheReader, CacheWriter, StreamingCache};
pub use traits::{
    Cache, CacheConfig as UnifiedCacheConfig, CacheEntry, CacheKey, CacheMetadata,
    CacheStatistics as UnifiedCacheStatistics,
};

// Legacy exports (deprecated - maintained for compatibility)
pub use config::{
    CacheConfig, CacheConfigBuilder, CacheConfigLoader, CacheConfigResolver, CacheConfiguration,
    GlobalCacheConfig, TaskCacheConfig,
};
pub use engine::CacheEngine;
pub use hashing::{expand_glob_pattern, HashEngine};
pub use item::CacheItem;
pub use mode::{get_cache_mode, CacheMode};

// Concurrent caching components
pub use concurrent::action::{ActionCache, ActionComponents, ActionDigest, ActionResult};
pub use concurrent::{ConcurrentCache, ConcurrentCacheBuilder};
pub use content_addressed_store::{ContentAddressedStore, ObjectMetadata};
pub use keys::{CacheKeyFilterConfig, CacheKeyGenerator, FilterStats};
pub use manager::{CacheManager, CacheStatistics};
pub use types::CachedTaskResult;

// Re-export security components
pub use security::{audit, capabilities, merkle, secure, signing};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Resolve a cache path relative to the cache directory
pub(crate) fn resolve_cache_path(cache_dir: &Path, path: impl AsRef<OsStr>) -> PathBuf {
    let path = PathBuf::from(path.as_ref());

    let mut resolved = if path.is_absolute() {
        path
    } else {
        cache_dir.join(path)
    };

    resolved.set_extension("json");
    resolved
}

/// Create a new cache using the unified implementation
///
/// This is the recommended way to create a cache for new code.
///
/// # Example
/// ```no_run
/// use cuenv::cache::{CacheBuilder, CacheConfiguration};
///
/// # async fn example() -> cuenv::cache::Result<()> {
/// // Create an async cache
/// let cache = CacheBuilder::new("/tmp/cache")
///     .with_config(CacheConfiguration::default())
///     .build_async()
///     .await?;
///
/// // Or create a sync cache
/// let sync_cache = CacheBuilder::new("/tmp/cache")
///     .build_sync()?;
/// # Ok(())
/// # }
/// ```
pub fn new_cache(base_dir: impl Into<PathBuf>) -> CacheBuilder {
    CacheBuilder::new(base_dir)
}
