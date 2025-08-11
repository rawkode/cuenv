//! Cache cleanup and maintenance utilities

use crate::core::internal::PathUtils;
use std::path::PathBuf;
use tokio::fs;
use tracing;

/// Cache cleanup operations
pub struct CacheCleanup;

impl CacheCleanup {
    /// Clean up files for a specific entry
    pub async fn cleanup_entry_files(base_dir: &PathBuf, key: &str) {
        let metadata_path = PathUtils::metadata_path(base_dir, key);
        let data_path = PathUtils::object_path(base_dir, key);

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

    /// Scan for corrupted files in the cache
    pub async fn scan_for_corrupted_files(base_dir: &PathBuf) -> Result<Vec<String>, std::io::Error> {
        let mut corrupted_files = Vec::new();
        let objects_dir = base_dir.join("objects");

        if !objects_dir.exists() {
            return Ok(corrupted_files);
        }

        let mut entries = match fs::read_dir(&objects_dir).await {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read objects directory: {}", e);
                return Ok(corrupted_files);
            }
        };

        while let Some(entry) = entries.next_entry().await.transpose() {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_file() {
                        // Check file integrity
                        match fs::File::open(&path).await {
                            Ok(mut file) => {
                                // Try to read the first few bytes
                                let mut buffer = vec![0; 64];
                                match tokio::io::AsyncReadExt::read(&mut file, &mut buffer).await {
                                    Ok(0) => {
                                        // Empty file is considered corrupted
                                        if let Some(name) = path.file_name() {
                                            if let Some(name_str) = name.to_str() {
                                                corrupted_files.push(name_str.to_string());
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        // File readable, check if it's valid bincode
                                        match fs::read(&path).await {
                                            Ok(data) => {
                                                // Try to deserialize metadata
                                                if bincode::deserialize::<serde_json::Value>(&data).is_err() {
                                                    if let Some(name) = path.file_name() {
                                                        if let Some(name_str) = name.to_str() {
                                                            corrupted_files.push(name_str.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to read file {}: {}", path.display(), e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to read file {}: {}", path.display(), e);
                                        if let Some(name) = path.file_name() {
                                            if let Some(name_str) = name.to_str() {
                                                corrupted_files.push(name_str.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to open file {}: {}", path.display(), e);
                                if let Some(name) = path.file_name() {
                                    if let Some(name_str) = name.to_str() {
                                        corrupted_files.push(name_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Error reading directory entry: {}", e);
                }
            }
        }

        Ok(corrupted_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_cleanup_nonexistent_files() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();

        // This should not panic even if files don't exist
        CacheCleanup::cleanup_entry_files(&base_dir, "nonexistent_key").await;
    }

    #[tokio::test]
    async fn test_scan_for_corrupted_files_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();

        let corrupted = CacheCleanup::scan_for_corrupted_files(&base_dir).await.unwrap();
        assert!(corrupted.is_empty());
    }

    #[tokio::test]
    async fn test_scan_for_corrupted_files_with_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();
        let objects_dir = base_dir.join("objects");
        fs::create_dir_all(&objects_dir).await.unwrap();

        // Create an empty file (should be considered corrupted)
        let empty_file = objects_dir.join("empty_file");
        fs::write(&empty_file, b"").await.unwrap();

        let corrupted = CacheCleanup::scan_for_corrupted_files(&base_dir).await.unwrap();
        assert_eq!(corrupted.len(), 1);
        assert_eq!(corrupted[0], "empty_file");
    }
}