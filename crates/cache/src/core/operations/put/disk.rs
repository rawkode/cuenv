//! Disk write operations for put

use crate::core::paths::{metadata_path, object_path};
use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result, StoreType};
use crate::traits::CacheMetadata;
use std::time::Duration;
use tokio::fs;

use super::super::utils::serialize;

impl Cache {
    /// Write data and metadata to disk
    pub(super) async fn write_to_disk(
        &self,
        key: &str,
        data: &[u8],
        metadata: &CacheMetadata,
    ) -> Result<()> {
        let data_path = object_path(&self.inner, key);
        let data_parent = match data_path.parent() {
            Some(p) => p,
            None => {
                return Err(CacheError::Configuration {
                    message: "Invalid cache path".to_string(),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check cache configuration".to_string(),
                    },
                });
            }
        };

        // Acquire write semaphore with timeout to prevent deadlocks
        let _permit = match tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.write_semaphore.acquire(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(CacheError::StoreUnavailable {
                    store_type: StoreType::Local,
                    reason: "Write semaphore closed".to_string(),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
            Err(_) => {
                return Err(CacheError::Timeout {
                    operation: "acquire write semaphore for put",
                    duration: Duration::from_secs(5),
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        };

        match fs::create_dir_all(data_parent).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: data_parent.to_path_buf(),
                    operation: "create cache directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: data_parent.to_path_buf(),
                    },
                });
            }
        }

        // Serialize metadata separately
        let metadata_bytes = match serialize(metadata) {
            Ok(bytes) => bytes,
            Err(e) => return Err(e),
        };

        // Write metadata to separate file for efficient scanning
        let metadata_path = metadata_path(&self.inner, key);
        let metadata_parent = match metadata_path.parent() {
            Some(p) => p,
            None => {
                return Err(CacheError::Configuration {
                    message: "Invalid metadata path".to_string(),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check cache configuration".to_string(),
                    },
                });
            }
        };

        match fs::create_dir_all(metadata_parent).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: metadata_parent.to_path_buf(),
                    operation: "create metadata directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: metadata_parent.to_path_buf(),
                    },
                });
            }
        }

        // CRITICAL FIX: Write data file FIRST, then metadata
        // This prevents readers from seeing metadata pointing to incomplete data

        let unique_id = uuid::Uuid::new_v4();
        let temp_data_path = data_path.with_extension(format!("tmp.{unique_id}"));
        let temp_metadata_path = metadata_path.with_extension(format!("tmp.{unique_id}"));

        // Step 1: Write data file first
        match fs::write(&temp_data_path, data).await {
            Ok(()) => {}
            Err(e) => {
                return Err(CacheError::Io {
                    path: temp_data_path.clone(),
                    operation: "write cache data file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: temp_data_path.clone(),
                    },
                });
            }
        }

        // Step 2: Write metadata file only after data is complete
        match fs::write(&temp_metadata_path, &metadata_bytes).await {
            Ok(()) => {}
            Err(e) => {
                // Clean up temp data file since metadata write failed
                let _ = fs::remove_file(&temp_data_path).await;
                return Err(CacheError::Io {
                    path: temp_metadata_path.clone(),
                    operation: "write metadata file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: temp_metadata_path.clone(),
                    },
                });
            }
        }

        // Step 3: Rename data file first (so metadata never points to missing data)
        match fs::rename(&temp_data_path, &data_path).await {
            Ok(()) => {}
            Err(e) => {
                // Clean up temp files
                let _ = fs::remove_file(&temp_metadata_path).await;
                let _ = fs::remove_file(&temp_data_path).await;
                return Err(CacheError::Io {
                    path: data_path.clone(),
                    operation: "rename cache data file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        // Step 4: Finally rename metadata file (data is now available)
        match fs::rename(&temp_metadata_path, &metadata_path).await {
            Ok(()) => {}
            Err(e) => {
                // Data file exists but metadata rename failed - clean up data file
                let _ = fs::remove_file(&data_path).await;
                let _ = fs::remove_file(&temp_metadata_path).await;
                return Err(CacheError::Io {
                    path: metadata_path.clone(),
                    operation: "rename metadata file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        // Record disk usage for quota tracking
        self.inner
            .memory_manager
            .record_disk_usage(&data_path, data.len() as i64);

        Ok(())
    }
}