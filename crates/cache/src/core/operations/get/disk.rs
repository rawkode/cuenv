//! Disk loading operations for get

use crate::core::paths::{metadata_path, object_path};
use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result, StoreType};
use crate::traits::CacheMetadata;
use serde::de::DeserializeOwned;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};
use tokio::fs;

use super::super::utils::deserialize;

impl Cache {
    pub(super) async fn load_from_disk<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let metadata_path = metadata_path(&self.inner, key);
        let data_path = object_path(&self.inner, key);

        // Acquire read semaphore with timeout to prevent deadlocks
        let permit = match tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.read_semaphore.acquire(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "I/O semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
            Err(_) => {
                return Err(CacheError::Timeout {
                    operation: "acquire read semaphore for get",
                    duration: Duration::from_secs(5),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        // Read metadata first
        let metadata_bytes = match fs::read(&metadata_path).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
            Err(e) => {
                self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                return Err(CacheError::Io {
                    path: metadata_path,
                    operation: "read metadata file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        let metadata: CacheMetadata = match deserialize(&metadata_bytes) {
            Ok(m) => m,
            Err(_e) => {
                // Release semaphore permit before cleanup to prevent deadlock
                drop(permit);

                // Metadata is corrupted - clean up both files to prevent future corruption errors
                let _ = fs::remove_file(&metadata_path).await;
                let _ = fs::remove_file(&data_path).await;
                self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                // Treat as cache miss for better error recovery
                return Ok(None);
            }
        };

        // Check if expired
        if let Some(expires_at) = metadata.expires_at {
            if expires_at <= SystemTime::now() {
                // Release semaphore permit before cleanup to prevent deadlock
                drop(permit);

                // Remove expired entry
                let _ = fs::remove_file(&metadata_path).await;
                let _ = fs::remove_file(&data_path).await;
                self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        }

        // Load and process data
        self.load_and_cache_data(key, data_path, metadata).await
    }
}