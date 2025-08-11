//! Cache clear operations

use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result};
use std::sync::atomic::Ordering;
use tokio::fs;

impl Cache {
    /// Clear all entries from the cache
    pub async fn clear(&self) -> Result<()> {
        // Clear memory cache
        self.inner.memory_cache.clear();
        // Clear fast path cache
        self.inner.fast_path.clear();
        self.inner.stats.total_bytes.store(0, Ordering::Relaxed);
        self.inner.stats.entry_count.store(0, Ordering::Relaxed);

        // Clear disk cache
        let objects_dir = self.inner.base_dir.join("objects");
        if fs::metadata(&objects_dir).await.is_ok() {
            match fs::remove_dir_all(&objects_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: objects_dir.clone(),
                        operation: "clear cache directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: objects_dir.clone(),
                        },
                    });
                }
            }

            match fs::create_dir_all(&objects_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: objects_dir.clone(),
                        operation: "recreate cache directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions { path: objects_dir },
                    });
                }
            }
        }

        // Also clear metadata directory
        let metadata_dir = self.inner.base_dir.join("metadata");
        if fs::metadata(&metadata_dir).await.is_ok() {
            match fs::remove_dir_all(&metadata_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: metadata_dir.clone(),
                        operation: "clear metadata directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: metadata_dir.clone(),
                        },
                    });
                }
            }

            match fs::create_dir_all(&metadata_dir).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: metadata_dir.clone(),
                        operation: "recreate metadata directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions { path: metadata_dir },
                    });
                }
            }
        }

        Ok(())
    }
}
