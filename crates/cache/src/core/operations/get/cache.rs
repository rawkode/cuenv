//! Caching operations for get

use crate::core::internal::InMemoryEntry;
use crate::core::paths::metadata_path;
use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheMetadata;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;

use super::super::utils::{deserialize, mmap_file};

impl Cache {
    pub(super) async fn load_and_cache_data<T>(
        &self,
        key: &str,
        data_path: std::path::PathBuf,
        metadata: CacheMetadata,
    ) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        // Try to memory-map the data file for zero-copy access
        let (mmap_option, data) = match mmap_file(&data_path) {
            Ok(mmap) => {
                let data = Vec::new(); // Empty vec, we'll use mmap
                (Some(mmap), data)
            }
            Err(_) => {
                // Fall back to regular file read
                match fs::read(&data_path).await {
                    Ok(bytes) => (None, bytes),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // Metadata exists but data doesn't - corrupted state
                        let metadata_path = metadata_path(&self.inner, key);
                        let _ = fs::remove_file(&metadata_path).await;
                        self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(CacheError::Corruption {
                            key: key.to_string(),
                            reason: "Metadata exists but data is missing".to_string(),
                            recovery_hint: RecoveryHint::ClearAndRetry,
                        });
                    }
                    Err(e) => {
                        self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(CacheError::Io {
                            path: data_path,
                            operation: "read cache data file",
                            source: e,
                            recovery_hint: RecoveryHint::Retry {
                                after: std::time::Duration::from_millis(100),
                            },
                        });
                    }
                }
            }
        };

        // Store in memory cache for hot access
        let mmap_arc = mmap_option.map(Arc::new);
        let size = if mmap_arc.is_some() {
            metadata.size_bytes
        } else {
            data.len() as u64
        };

        let can_store_in_memory = match self.inner.config.max_memory_size {
            Some(max) => {
                let current = self.inner.stats.total_bytes.load(Ordering::Relaxed);
                current.saturating_add(size) <= max
            }
            None => true,
        };

        if can_store_in_memory {
            let entry = Arc::new(InMemoryEntry {
                mmap: mmap_arc.clone(),
                data: data.clone(),
                metadata: metadata.clone(),
                last_accessed: RwLock::new(Instant::now()),
            });

            self.inner
                .memory_cache
                .insert(key.to_string(), entry.clone());

            // Record access for eviction policy and update memory stats
            self.inner.eviction_policy.on_access(key, size);
            self.inner
                .stats
                .total_bytes
                .fetch_add(size, Ordering::Relaxed);
        }

        // Count this as a hit regardless of whether we cached in memory
        self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);

        // Deserialize from the appropriate source
        let data_slice = if let Some(ref mmap) = mmap_arc {
            &mmap.as_ref()[..]
        } else {
            &data
        };

        match deserialize::<T>(data_slice) {
            Ok(value) => Ok(Some(value)),
            Err(_e) => {
                // Remove from memory cache as well
                self.inner.memory_cache.remove(key);
                self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                // Return None (cache miss) instead of error for better recovery
                Ok(None)
            }
        }
    }
}
