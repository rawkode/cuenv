use crate::{get_cache_mode, CacheItem, CacheMode, HashEngine};
use cuenv_core::{Error, Result};
use cuenv_utils::atomic_file::write_atomic_string;
use cuenv_utils::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Main cache engine for cuenv
#[derive(Debug)]
pub struct CacheEngine {
    /// The cache directory (e.g., ~/.cache/cuenv)
    pub cache_dir: PathBuf,

    /// Hash engine for content-based caching
    pub hash: HashEngine,

    /// Current cache mode (immutable once set)
    mode: CacheMode,
}

impl CacheEngine {
    /// Create a new cache engine
    pub fn new() -> Result<CacheEngine> {
        let cache_dir = XdgPaths::cache_dir();

        log::debug!("Creating cache engine with cache_dir: {cache_dir:?}");

        // Create cache directory if it doesn't exist
        fs::create_dir_all(&cache_dir)
            .map_err(|e| Error::file_system(cache_dir.clone(), "create cache directory", e))?;

        // Create cache directory tag for tools that understand them
        let cache_tag = cache_dir.join("CACHEDIR.TAG");
        if !cache_tag.exists() {
            let tag_content = r#"Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cuenv.
# For information see https://bford.info/cachedir"#;

            write_atomic_string(&cache_tag, tag_content)?;
        }

        let hash = HashEngine::new(&cache_dir)?;

        Ok(CacheEngine {
            cache_dir,
            hash,
            mode: get_cache_mode(),
        })
    }

    /// Create a new cache engine with a specific mode
    pub fn with_mode(mode: CacheMode) -> Result<CacheEngine> {
        let cache_dir = XdgPaths::cache_dir();

        log::debug!("Creating cache engine with cache_dir: {cache_dir:?}, mode: {mode:?}");

        // Create cache directory if it doesn't exist
        fs::create_dir_all(&cache_dir)
            .map_err(|e| Error::file_system(cache_dir.clone(), "create cache directory", e))?;

        // Create cache directory tag for tools that understand them
        let cache_tag = cache_dir.join("CACHEDIR.TAG");
        if !cache_tag.exists() {
            let tag_content = r#"Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cuenv.
# For information see https://bford.info/cachedir"#;

            write_atomic_string(&cache_tag, tag_content)?;
        }

        let hash = HashEngine::new(&cache_dir)?;

        Ok(CacheEngine {
            cache_dir,
            hash,
            mode,
        })
    }

    /// Create a cache item for the given path
    pub fn cache<T>(&self, path: impl AsRef<Path>) -> Result<CacheItem<T>>
    where
        T: Default + for<'de> Deserialize<'de> + Serialize,
    {
        CacheItem::load(&self.cache_dir, path)
    }

    /// Clean up stale cache entries older than the specified duration
    pub fn clean_stale_cache(&self, lifetime: Duration) -> Result<(usize, u64)> {
        log::debug!("Cleaning up stale cached artifacts older than {lifetime:?}");

        let mut files_deleted = 0;
        let mut bytes_saved = 0;

        // Clean hash manifests
        if self.hash.hashes_dir.exists() {
            let result = self.remove_stale_files(&self.hash.hashes_dir, lifetime)?;
            files_deleted += result.0;
            bytes_saved += result.1;
        }

        // Clean task cache files
        let tasks_dir = self.cache_dir.join("tasks");
        if tasks_dir.exists() {
            let result = self.remove_stale_files(&tasks_dir, lifetime)?;
            files_deleted += result.0;
            bytes_saved += result.1;
        }

        log::debug!("Deleted {files_deleted} artifacts and saved {bytes_saved} bytes");

        Ok((files_deleted, bytes_saved))
    }

    /// Clear all cache entries
    pub fn clear(&self) -> Result<()> {
        log::debug!("Clearing all cache entries");

        if self.cache_dir.exists() {
            // Remove contents but keep the directory itself
            let entries = fs::read_dir(&self.cache_dir).map_err(|e| {
                Error::file_system(self.cache_dir.clone(), "read cache directory", e)
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    Error::file_system(self.cache_dir.clone(), "read cache directory entry", e)
                })?;

                let path = entry.path();
                if path.file_name().unwrap_or_default() == "CACHEDIR.TAG" {
                    continue; // Keep the cache directory tag
                }

                if path.is_dir() {
                    fs::remove_dir_all(&path)
                        .map_err(|e| Error::file_system(path, "remove cache subdirectory", e))?;
                } else {
                    fs::remove_file(&path)
                        .map_err(|e| Error::file_system(path, "remove cache file", e))?;
                }
            }
        }

        Ok(())
    }

    /// Check if cache is readable
    pub fn is_readable(&self) -> bool {
        self.mode.is_readable()
    }

    /// Check if cache is read-only
    pub fn is_read_only(&self) -> bool {
        self.mode.is_read_only()
    }

    /// Check if cache is writable
    pub fn is_writable(&self) -> bool {
        self.mode.is_writable()
    }

    /// Check if cache is write-only
    pub fn is_write_only(&self) -> bool {
        self.mode.is_write_only()
    }

    /// Get the current cache mode
    pub fn mode(&self) -> CacheMode {
        self.mode
    }

    /// Remove files older than the specified duration
    fn remove_stale_files(&self, dir: &Path, max_age: Duration) -> Result<(usize, u64)> {
        let mut files_deleted = 0;
        let mut bytes_saved = 0;

        let cutoff_time = std::time::SystemTime::now() - max_age;

        let entries = fs::read_dir(dir)
            .map_err(|e| Error::file_system(dir.to_path_buf(), "read directory for cleanup", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                Error::file_system(dir.to_path_buf(), "read directory entry for cleanup", e)
            })?;

            let path = entry.path();
            let metadata = entry.metadata().map_err(|e| {
                Error::file_system(path.clone(), "read file metadata for cleanup", e)
            })?;

            if let Ok(modified) = metadata.modified() {
                if modified < cutoff_time {
                    let file_size = metadata.len();

                    if path.is_dir() {
                        fs::remove_dir_all(&path)
                            .map_err(|e| Error::file_system(path, "remove stale directory", e))?;
                    } else {
                        fs::remove_file(&path)
                            .map_err(|e| Error::file_system(path, "remove stale file", e))?;
                    }

                    files_deleted += 1;
                    bytes_saved += file_size;
                }
            }
        }

        Ok((files_deleted, bytes_saved))
    }
}
