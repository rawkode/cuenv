//! Compression configuration and utilities
//!
//! This module handles data compression settings and statistics
//! for the cache storage backend.

use super::format::DEFAULT_COMPRESSION_LEVEL;

/// Compression configuration
#[derive(Debug, Clone, Copy)]
pub struct CompressionConfig {
    /// Whether compression is enabled
    pub enabled: bool,
    /// Compression level (1-22 for zstd, default 3)
    pub level: i32,
    /// Minimum size in bytes before compression is applied
    pub min_size: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: DEFAULT_COMPRESSION_LEVEL,
            min_size: 1024, // Don't compress files smaller than 1KB
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub enabled: bool,
    pub level: i32,
    pub min_size: usize,
}