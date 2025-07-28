//! Cache configuration types
use super::CacheMode;
use std::path::PathBuf;

/// Configuration for cache systems
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Base directory for cache storage
    pub base_dir: PathBuf,
    /// Maximum cache size in bytes
    pub max_size: u64,
    /// Cache mode (read-only, read-write, etc.)
    pub mode: CacheMode,
    /// Threshold for inline storage optimization (bytes)
    pub inline_threshold: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        // Use XDG cache directory which respects XDG_CACHE_HOME
        use crate::xdg::XdgPaths;
        Self {
            base_dir: XdgPaths::cache_dir(),
            max_size: 10 * 1024 * 1024 * 1024, // 10GB
            mode: CacheMode::ReadWrite,
            inline_threshold: 1024, // 1KB
        }
    }
}
