//! Cache cleanup operations

mod background;

pub use background::start_cleanup_task;

use crate::errors::Result;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;

use super::paths::{metadata_path, object_path, object_path_from_hash};
use super::types::{Cache, CacheInner};

impl Cache {
    /// Clean up expired entries and corrupted files
    pub(super) async fn cleanup_expired_entries(inner: &Arc<CacheInner>) -> Result<()> {
        let now = SystemTime::now();
        let mut expired_keys = Vec::new();
        let mut corrupted_keys = Vec::new();

        // Find expired entries in memory
        for entry in inner.memory_cache.iter() {
            if let Some(expires_at) = entry.value().metadata.expires_at {
                if expires_at <= now {
                    expired_keys.push(entry.key().clone());
                }
            }
        }

        // Scan disk for orphaned metadata files (no corresponding data file)
        let metadata_dir = inner.base_dir.join("metadata");
        if fs::metadata(&metadata_dir).await.is_ok() {
            Self::scan_for_corrupted_files(inner, &metadata_dir, &mut corrupted_keys).await;
        }

        // Remove expired entries
        for key in expired_keys {
            if let Some((_, entry)) = inner.memory_cache.remove(&key) {
                inner
                    .stats
                    .total_bytes
                    .fetch_sub(entry.data.len() as u64, Ordering::Relaxed);
                inner.stats.entry_count.fetch_sub(1, Ordering::Relaxed);
                inner.stats.expired_cleanups.fetch_add(1, Ordering::Relaxed);
            }

            Self::cleanup_entry_files(inner, &key).await;
        }

        // Remove corrupted entries
        for key in corrupted_keys {
            // Remove from memory cache if present
            inner.memory_cache.remove(&key);
            Self::cleanup_entry_files(inner, &key).await;
            inner.stats.errors.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Scan for corrupted files in the top level only (non-recursive for now)
    async fn scan_for_corrupted_files(
        inner: &Arc<CacheInner>,
        metadata_dir: &std::path::Path,
        _corrupted_keys: &mut [String],
    ) {
        // Simple non-recursive cleanup to avoid boxing issues
        // Just clean up obvious orphaned files in the metadata directory
        let mut read_dir = match tokio::fs::read_dir(metadata_dir).await {
            Ok(dir) => dir,
            Err(_) => return,
        };

        let mut cleanup_count = 0;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if !path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("meta") {
                // Check if corresponding data file exists
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let data_path = object_path_from_hash(inner, file_stem);
                    let data_exists = fs::metadata(&data_path).await.is_ok();
                    if !data_exists {
                        // Orphaned metadata file - clean it up
                        if fs::remove_file(&path).await.is_ok() {
                            cleanup_count += 1;
                            tracing::debug!(
                                "Cleaned up orphaned metadata file: {}",
                                path.display()
                            );
                        }
                    }
                }
            }

            // Limit cleanup per run to avoid blocking too long
            if cleanup_count >= 50 {
                break;
            }
        }
    }

    /// Clean up files for a specific entry
    pub(super) async fn cleanup_entry_files(inner: &Arc<CacheInner>, key: &str) {
        let metadata_path = metadata_path(inner, key);
        let data_path = object_path(inner, key);

        match fs::remove_file(&metadata_path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(
                    "Failed to remove expired metadata {}: {}",
                    metadata_path.display(),
                    e
                );
            }
        }

        match fs::remove_file(&data_path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(
                    "Failed to remove expired data {}: {}",
                    data_path.display(),
                    e
                );
            }
        }
    }
}