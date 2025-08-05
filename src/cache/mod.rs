//! High-performance caching system for cuenv tasks
//!
//! This module provides a unified, production-grade caching infrastructure
//! with clean architecture and proper async/sync boundaries.

// Core modules
mod bridge;
mod cache_impl;
mod cache_warming;
mod errors;
mod eviction;
mod fast_path;
mod memory_manager;
mod metrics;
mod metrics_endpoint;
mod monitored_cache;
mod monitoring;
mod performance;
mod serde_helpers;
mod storage_backend;
mod streaming;
mod traits;

// Legacy modules (to be migrated)
mod config;
mod configuration;
mod engine;
mod hash_engine;
mod item;
mod key_generator;
mod mode;
pub mod signing;
mod types;

// Security modules (Phase 7)
pub mod audit;
pub mod capabilities;
pub mod merkle;
pub mod secure_cache;

// Reliability and production features (Phase 9)
// pub mod health_endpoint; // Disabled - missing ProductionHardening type
pub mod reliability;

// Advanced features (Phase 10)
// pub mod analytics_dashboard; // Disabled - requires axum dependency
// pub mod multi_tenant; // Disabled - missing CacheCapability type
pub mod platform_optimizations;
pub mod predictive_cache;

// Legacy implementations (deprecated - will be removed)
mod action_cache;
mod cache_manager;
mod concurrent_cache;
mod content_addressed_store;

// Public exports - new unified API
pub use bridge::{CacheBuilder, SyncCache};
pub use cache_impl::Cache as ProductionCache;
pub use errors::{CacheError, RecoveryHint, Result as CacheResult};
pub use metrics_endpoint::MetricsEndpoint;
pub use monitored_cache::{MonitoredCache, MonitoredCacheBuilder};
pub use monitoring::{CacheMonitor, HitRateReport, RealTimeStatsReport, TracedOperation};
pub use storage_backend::{CompressionConfig, StorageBackend};
pub use streaming::{CacheReader, CacheWriter, StreamingCache};
pub use traits::{
    Cache, CacheConfig as UnifiedCacheConfig, CacheEntry, CacheKey, CacheMetadata,
    CacheStatistics as UnifiedCacheStatistics,
};

// Legacy exports (deprecated - maintained for compatibility)
pub use config::CacheConfig;
pub use configuration::{
    CacheConfigBuilder, CacheConfigLoader, CacheConfigResolver, CacheConfiguration,
    GlobalCacheConfig, TaskCacheConfig,
};
pub use engine::CacheEngine;
pub use hash_engine::{expand_glob_pattern, HashEngine};
pub use item::CacheItem;
pub use mode::{get_cache_mode, CacheMode};

// Legacy advanced caching components (deprecated)
pub use action_cache::{ActionCache, ActionComponents, ActionDigest, ActionResult};
pub use cache_manager::{CacheManager, CacheStatistics};
pub use concurrent_cache::{ConcurrentCache, ConcurrentCacheBuilder};
pub use content_addressed_store::{ContentAddressedStore, ObjectMetadata};
pub use key_generator::{CacheKeyFilterConfig, CacheKeyGenerator, FilterStats};
pub use types::CachedTaskResult;

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
