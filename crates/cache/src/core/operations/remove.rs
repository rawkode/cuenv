//! Cache remove operations

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheKey;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::fs;

use super::super::paths::{metadata_path, object_path};
use super::super::types::Cache;

impl Cache {
    /// Remove an entry from the cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        let mut removed = false;

        // Remove from memory
        if let Some((_, entry)) = self.inner.memory_cache.remove(key) {
            let size = if entry.mmap.is_some() {
                entry.metadata.size_bytes
            } else {
                entry.data.len() as u64
            };

            // Record removal for eviction policy
            self.inner.eviction_policy.on_remove(key, size);

            self.inner
                .stats
                .total_bytes
                .fetch_sub(size, Ordering::Relaxed);
            self.inner.stats.entry_count.fetch_sub(1, Ordering::Relaxed);
            removed = true;
        }

        // Remove from fast-path cache
        if self.inner.fast_path.remove_small(key) {
            removed = true;
        }

        // Remove from disk (both metadata and data)
        let metadata_path = metadata_path(&self.inner, key);
        let data_path = object_path(&self.inner, key);

        match fs::remove_file(&metadata_path).await {
            Ok(()) => {
                // Only decrement entry count if it wasn't already removed from memory
                if !removed {
                    self.inner.stats.entry_count.fetch_sub(1, Ordering::Relaxed);
                }
                removed = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: metadata_path,
                    operation: "remove metadata file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        }

        // Get file size before removal for disk tracking
        let file_size = match tokio::fs::metadata(&data_path).await {
            Ok(metadata) => metadata.len() as i64,
            Err(_) => 0, // File doesn't exist or can't get metadata
        };

        match fs::remove_file(&data_path).await {
            Ok(()) => {
                // Record negative disk usage for removed file
                if file_size > 0 {
                    self.inner
                        .memory_manager
                        .record_disk_usage(&data_path, -file_size);
                }
                removed = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: data_path,
                    operation: "remove cache data file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        }

        if removed {
            self.inner.stats.removals.fetch_add(1, Ordering::Relaxed);
        }

        Ok(removed)
    }
}
