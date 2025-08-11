//! Cache builder and initialization

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::eviction::create_eviction_policy;
use crate::fast_path::FastPathCache;
use crate::memory_manager::{MemoryManager, MemoryThresholds};
use crate::traits::CacheConfig;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::sync::Semaphore;

use super::cleanup::start_cleanup_task;
use super::internal::CacheStats;
use super::types::{Cache, CacheInner};

impl Cache {
    /// Create a new unified cache with production-ready features
    pub async fn new(base_dir: PathBuf, config: CacheConfig) -> Result<Self> {
        // Create cache directories
        match fs::create_dir_all(&base_dir).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: base_dir.clone(),
                    operation: "create cache directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: base_dir.clone(),
                    },
                });
            }
        }

        // Create objects directory with 4-level sharding
        let objects_dir = base_dir.join("objects");
        match fs::create_dir_all(&objects_dir).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: objects_dir.clone(),
                    operation: "create objects directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: objects_dir },
                });
            }
        }

        // Create metadata directory structure
        let metadata_dir = base_dir.join("metadata");
        match fs::create_dir_all(&metadata_dir).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: metadata_dir.clone(),
                    operation: "create metadata directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: metadata_dir },
                });
            }
        }

        // Create eviction policy
        let eviction_policy = match create_eviction_policy(
            config.eviction_policy.as_deref().unwrap_or("lru"),
            config.max_memory_size.unwrap_or(1024 * 1024 * 1024), // 1GB default
        ) {
            Ok(policy) => policy,
            Err(e) => return Err(e),
        };

        // Create memory manager
        let memory_manager = Arc::new(MemoryManager::new(
            base_dir.clone(),
            config.max_disk_size.unwrap_or(10 * 1024 * 1024 * 1024), // 10GB default
            MemoryThresholds::default(),
        ));

        // Start memory monitoring
        let manager_clone = Arc::clone(&memory_manager);
        manager_clone.start_monitoring();

        let inner = Arc::new(CacheInner {
            config,
            base_dir,
            memory_cache: DashMap::new(),
            fast_path: FastPathCache::new(1024, 10000), // 1KB threshold, 10k entries
            eviction_policy,
            memory_manager,
            stats: CacheStats {
                hits: std::sync::atomic::AtomicU64::new(0),
                misses: std::sync::atomic::AtomicU64::new(0),
                writes: std::sync::atomic::AtomicU64::new(0),
                removals: std::sync::atomic::AtomicU64::new(0),
                errors: std::sync::atomic::AtomicU64::new(0),
                total_bytes: std::sync::atomic::AtomicU64::new(0),
                expired_cleanups: std::sync::atomic::AtomicU64::new(0),
                entry_count: std::sync::atomic::AtomicU64::new(0),
                stats_since: SystemTime::now(),
            },
            read_semaphore: Semaphore::new(200), // More permits for reads
            write_semaphore: Semaphore::new(50), // Fewer permits for writes
            cleanup_handle: RwLock::new(None),
            version: 3, // Version 3 with streaming and performance optimizations
        });

        let cache = Self { inner };

        // Start background cleanup task
        start_cleanup_task(&cache);

        Ok(cache)
    }
}