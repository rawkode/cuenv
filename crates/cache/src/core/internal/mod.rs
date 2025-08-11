//! Internal structures and utilities for the unified cache implementation

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheMetadata;
use memmap2::{Mmap, MmapOptions};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

/// Internal in-memory cache entry with memory mapping support
pub struct InMemoryEntry {
    /// Memory-mapped data for zero-copy access
    pub mmap: Option<Arc<Mmap>>,
    /// Raw data (used when mmap is not available)
    pub data: Vec<u8>,
    pub metadata: CacheMetadata,
    pub last_accessed: RwLock<Instant>,
}

/// Internal cache statistics with atomic counters
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub writes: AtomicU64,
    pub removals: AtomicU64,
    pub errors: AtomicU64,
    pub total_bytes: AtomicU64,
    pub expired_cleanups: AtomicU64,
    pub entry_count: AtomicU64,
    pub stats_since: SystemTime,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            removals: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            expired_cleanups: AtomicU64::new(0),
            entry_count: AtomicU64::new(0),
            stats_since: SystemTime::now(),
        }
    }
}

impl CacheStats {
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_write(&self, size: u64) {
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(size, Ordering::Relaxed);
        self.entry_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_removal(&self, size: u64) {
        self.removals.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_sub(size, Ordering::Relaxed);
        if self.entry_count.load(Ordering::Relaxed) > 0 {
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

/// Path utilities for the cache
pub struct PathUtils;

impl PathUtils {
    /// Generate object path from base directory and key hash
    pub fn object_path_from_hash(base_dir: &Path, hash: &str) -> PathBuf {
        let prefix = &hash[..2];
        let subdir = &hash[2..4];
        base_dir
            .join("objects")
            .join(prefix)
            .join(subdir)
            .join(hash)
    }

    /// Generate object path from base directory and key
    pub fn object_path(base_dir: &Path, key: &str) -> PathBuf {
        let hash = Self::hash_key(key);
        Self::object_path_from_hash(base_dir, &hash)
    }

    /// Generate metadata path from base directory and key
    pub fn metadata_path(base_dir: &Path, key: &str) -> PathBuf {
        let hash = Self::hash_key(key);
        let prefix = &hash[..2];
        let subdir = &hash[2..4];
        base_dir
            .join("metadata")
            .join(prefix)
            .join(subdir)
            .join(format!("{hash}.meta"))
    }

    /// Hash a cache key using SHA-256
    pub fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Memory mapping utilities
pub struct MemoryMapUtils;

impl MemoryMapUtils {
    /// Create a memory map for the given file path
    pub fn mmap_file(path: &PathBuf) -> Result<Mmap> {
        let file = std::fs::File::open(path).map_err(|e| CacheError::Io {
            path: path.clone(),
            operation: "open file for memory mapping",
            source: e,
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check if file exists and permissions are correct".to_string(),
            },
        })?;

        unsafe {
            MmapOptions::new().map(&file).map_err(|e| CacheError::Io {
                path: path.clone(),
                operation: "memory map file",
                source: e,
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check file size and system memory limits".to_string(),
                },
            })
        }
    }
}
