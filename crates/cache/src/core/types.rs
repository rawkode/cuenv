//! Core cache types and structures

use crate::eviction::EvictionPolicy;
use crate::fast_path::FastPathCache;
use crate::memory_manager::MemoryManager;
use crate::traits::CacheConfig;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use super::internal::{CacheStats, InMemoryEntry};

/// Production-ready unified cache implementation
#[derive(Clone)]
pub struct Cache {
    pub(super) inner: Arc<CacheInner>,
}

pub(super) struct CacheInner {
    /// Configuration
    pub config: CacheConfig,
    /// Base directory for file-based cache
    pub base_dir: PathBuf,
    /// In-memory cache for hot data
    pub memory_cache: DashMap<String, Arc<InMemoryEntry>>,
    /// Fast path cache for small values
    pub fast_path: FastPathCache,
    /// Eviction policy
    pub eviction_policy: Box<dyn EvictionPolicy>,
    /// Memory manager
    pub memory_manager: Arc<MemoryManager>,
    /// Statistics
    pub stats: CacheStats,
    /// Semaphore for limiting concurrent read operations
    pub read_semaphore: Semaphore,
    /// Semaphore for limiting concurrent write operations
    pub write_semaphore: Semaphore,
    /// Background cleanup task handle
    pub cleanup_handle: RwLock<Option<JoinHandle<()>>>,
    /// Cache format version
    pub version: u32,
}

impl Drop for CacheInner {
    fn drop(&mut self) {
        // Cancel cleanup task
        // parking_lot RwLock doesn't block on drop, so this is safe
        if let Some(handle) = self.cleanup_handle.write().take() {
            handle.abort();
        }
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("base_dir", &self.inner.base_dir)
            .field("version", &self.inner.version)
            .field("entry_count", &self.inner.memory_cache.len())
            .finish()
    }
}
