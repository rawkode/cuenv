//! High-performance streaming cache APIs for Google-scale operations
//!
//! This module provides zero-copy streaming interfaces for cache operations,
//! enabling efficient handling of large objects without memory overhead.
//!
//! Key features:
//! - AsyncRead/AsyncWrite trait implementations for streaming I/O
//! - Zero-copy transfers using sendfile/splice on Linux
//! - Chunked transfer encoding for network operations
//! - Memory-mapped streaming for hot data paths
//! - Vectored I/O for scatter-gather operations

use futures::io::{AsyncRead, AsyncWrite};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::errors::Result;

// Re-export all public types and traits
pub use reader::CacheReader;
pub use writer::CacheWriter;

// Module declarations
mod finalization;
pub mod operations;
mod path_utils;
mod reader;
mod writer;

#[cfg(test)]
mod tests;

/// Streaming cache operations trait
///
/// This trait extends the base cache with high-performance streaming operations
/// for handling large objects efficiently without loading them into memory.
pub trait StreamingCache: Send + Sync {
    /// Get a reader for streaming a cached value
    ///
    /// Returns a stream that can be read from without loading the entire
    /// value into memory. Perfect for large files or network transfers.
    fn get_reader<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<CacheReader>>> + Send + 'a>>;

    /// Get a writer for streaming a value into the cache
    ///
    /// Returns a writer that streams data directly to the cache storage
    /// without buffering the entire value in memory.
    fn get_writer<'a>(
        &'a self,
        key: &'a str,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<CacheWriter>> + Send + 'a>>;

    /// Copy a stream directly into the cache
    ///
    /// High-performance copy operation that uses platform-specific zero-copy
    /// mechanisms (sendfile, splice) when available.
    fn put_stream<'a, R>(
        &'a self,
        key: &'a str,
        reader: R,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>
    where
        R: AsyncRead + Send + 'a;

    /// Stream a cached value to a writer
    ///
    /// Efficiently copies cached data to the provided writer using zero-copy
    /// operations when possible.
    fn get_stream<'a, W>(
        &'a self,
        key: &'a str,
        writer: W,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>>> + Send + 'a>>
    where
        W: AsyncWrite + Send + 'a;
}

// Re-export operations for backward compatibility and convenience
pub use operations::vectored;
#[cfg(target_os = "linux")]
pub use operations::zero_copy;
