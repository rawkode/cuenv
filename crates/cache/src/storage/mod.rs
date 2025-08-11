//! Phase 2: Storage Backend Implementation
//!
//! This module provides a production-grade storage backend with:
//! - Binary format with bincode serialization
//! - Zstd compression for all cached data
//! - Write-ahead log for crash recovery
//! - CRC32C checksums for corruption detection
//! - Atomic multi-file updates
//! - Zero-copy operations where possible

pub mod backend;
mod compression;
mod format;
mod tests;
mod transaction;
mod wal;

// Re-export public types
pub use backend::StorageBackend;
pub use compression::{CompressionConfig, CompressionStats};
pub use format::{StorageHeader, CACHE_MAGIC, DEFAULT_COMPRESSION_LEVEL, MAX_WAL_SIZE, STORAGE_VERSION};
pub use wal::{WalEntry, WalOperation};