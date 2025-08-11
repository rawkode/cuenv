//! Cache system for cuenv
//!
//! This crate provides a comprehensive caching system with features like:
//! - Content-addressed storage
//! - Concurrent access
//! - Security features (signing, auditing)
//! - Performance monitoring
//! - Eviction policies
//! - Streaming support

pub mod bridge;
pub mod cleanup;
pub mod concurrent;
pub mod config;
pub mod content_addressed_store;
pub mod core;
pub mod engine;
pub mod entry;
pub mod errors;
pub mod eviction;
pub mod fast_path;
pub mod hashing;
pub mod health_endpoint;
pub mod item;
pub mod keys;
pub mod manager;
pub mod memory_manager;
pub mod metrics;
pub mod mode;
pub mod monitored;
pub mod monitoring;
pub mod performance;
pub mod security;
pub mod serialization;
pub mod storage;
pub mod streaming;
pub mod traits;
pub mod types;
pub mod warming;

// Re-export main types and traits selectively to avoid conflicts
pub use config::CacheConfig;
pub use core::Cache;
pub use errors::{CacheError, Error, Result};
pub use traits::CacheEntry;
pub use types::*;

// Re-export other modules without conflicts
pub use bridge::*;
pub use concurrent::*;
pub use content_addressed_store::*;
pub use engine::*;
pub use eviction::*;
pub use fast_path::*;
pub use hashing::*;
pub use health_endpoint::*;
pub use item::*;
pub use keys::*;
pub use manager::CacheManager;
pub use memory_manager::MemoryManager;
pub use metrics::*;
pub use mode::*;
pub use monitored::MonitoredCache;
pub use monitoring::CacheMonitor;
pub use performance::*;
pub use security::*;
pub use serialization::*;
pub use storage::*;
pub use streaming::*;
pub use warming::CacheWarmer;

use std::path::{Path, PathBuf};

/// Helper function to resolve cache paths
pub fn resolve_cache_path(cache_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cache_dir.join(path)
    }
}

// NOTE: new_cache function is available in the mod module but has dependency conflicts
