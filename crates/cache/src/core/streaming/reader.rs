//! Streaming read operations

use crate::errors::{CacheError, RecoveryHint, Result, StoreType};
use crate::streaming::CacheReader;
use crate::traits::CacheKey;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;

use crate::core::operations::utils::deserialize;
use crate::core::paths::{metadata_path, object_path};
use crate::core::types::Cache;

impl Cache {
    pub fn get_reader<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<CacheReader>>> + Send + 'a>> {
        Box::pin(async move {
            match key.validate() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            // Check if the entry exists and get metadata
            let metadata_path = metadata_path(&self.inner, key);
            let data_path = object_path(&self.inner, key);

            // Acquire read semaphore with timeout to prevent deadlocks
            let _permit = match tokio::time::timeout(
                Duration::from_secs(5),
                self.inner.read_semaphore.acquire(),
            )
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
                        operation: "acquire read semaphore for streaming",
                        duration: Duration::from_secs(5),
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(100),
                        },
                    });
                }
            };

            // Read metadata
            let metadata_bytes = match fs::read(&metadata_path).await {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(None);
                }
                Err(e) => {
                    return Err(CacheError::Io {
                        path: metadata_path,
                        operation: "read metadata for streaming",
                        source: e,
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(100),
                        },
                    });
                }
            };

            let metadata: crate::traits::CacheMetadata = match deserialize(&metadata_bytes) {
                Ok(m) => m,
                Err(e) => return Err(e),
            };

            // Check if expired
            if let Some(expires_at) = metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    // Remove expired entry
                    let _ = fs::remove_file(&metadata_path).await;
                    let _ = fs::remove_file(&data_path).await;
                    return Ok(None);
                }
            }

            // Update access time in memory cache if present
            if let Some(entry) = self.inner.memory_cache.get(key) {
                *entry.last_accessed.write() = Instant::now();
            }

            // Prefer memory-mapped reader for performance
            #[cfg(target_os = "linux")]
            {
                match CacheReader::from_mmap(data_path.clone(), metadata.clone()).await {
                    Ok(reader) => return Ok(Some(reader)),
                    Err(_) => {
                        // Fall back to regular file reader
                    }
                }
            }

            // Use file-based reader
            match CacheReader::from_file(data_path, metadata).await {
                Ok(reader) => Ok(Some(reader)),
                Err(e) => Err(e),
            }
        })
    }
}
