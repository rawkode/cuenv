//! Miscellaneous cache operations

mod clear;
mod stats;

use crate::core::paths::metadata_path;
use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result, StoreType};
use crate::traits::{CacheKey, CacheMetadata};
use std::time::{Duration, SystemTime};
use tokio::fs;

use super::utils::deserialize;

impl Cache {
    /// Check if a key exists in the cache
    pub async fn contains(&self, key: &str) -> Result<bool> {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Check fast-path cache first
        if self.inner.fast_path.contains_small(key) {
            return Ok(true);
        }

        // Check memory cache
        if let Some(entry) = self.inner.memory_cache.get(key) {
            // Check if expired
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        // Check disk - only need to check metadata
        let metadata_path = metadata_path(&self.inner, key);

        // Use async metadata check instead of sync exists()
        let exists = match fs::metadata(&metadata_path).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                return Err(CacheError::Io {
                    path: metadata_path.clone(),
                    operation: "check metadata file existence",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        if !exists {
            return Ok(false);
        }

        // Need to check if the disk entry is expired
        // Acquire read semaphore with timeout to prevent deadlocks
        let _permit =
            match tokio::time::timeout(Duration::from_secs(5), self.inner.read_semaphore.acquire())
                .await
            {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    return Err(CacheError::StoreUnavailable {
                        store_type: StoreType::Local,
                        reason: "Read semaphore closed".to_string(),
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(100),
                        },
                    });
                }
                Err(_) => {
                    return Err(CacheError::Timeout {
                        operation: "acquire read semaphore for contains",
                        duration: Duration::from_secs(5),
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(100),
                        },
                    });
                }
            };

        match fs::read(&metadata_path).await {
            Ok(metadata_bytes) => {
                let metadata: CacheMetadata = match deserialize(&metadata_bytes) {
                    Ok(m) => m,
                    Err(_) => return Ok(false),
                };

                // Check if expired
                if let Some(expires_at) = metadata.expires_at {
                    if expires_at <= SystemTime::now() {
                        // It's expired, so it doesn't exist
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Get metadata for a cache entry
    pub async fn metadata(&self, key: &str) -> Result<Option<CacheMetadata>> {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Check fast-path cache first
        if let Some((_data, metadata)) = self.inner.fast_path.get_small(key) {
            return Ok(Some(metadata));
        }

        // Check memory cache
        if let Some(entry) = self.inner.memory_cache.get(key) {
            return Ok(Some(entry.metadata.clone()));
        }

        // Try to load from disk (just metadata)
        let metadata_path = metadata_path(&self.inner, key);
        match fs::read(&metadata_path).await {
            Ok(metadata_bytes) => {
                let metadata: CacheMetadata = match deserialize(&metadata_bytes) {
                    Ok(m) => m,
                    Err(e) => return Err(e),
                };

                Ok(Some(metadata))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CacheError::Io {
                path: metadata_path,
                operation: "read metadata file",
                source: e,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            }),
        }
    }
}
