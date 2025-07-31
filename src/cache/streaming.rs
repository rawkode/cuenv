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

use crate::cache::errors::{CacheError, RecoveryHint, Result};
use crate::cache::traits::{CacheKey, CacheMetadata};
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use parking_lot::RwLock;
use pin_project_lite::pin_project;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};
use tokio::fs::File;
<<<<<<< HEAD
use tokio::io::AsyncWriteExt as TokioAsyncWriteExt;
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf};
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite, ReadBuf};
=======
use tokio::io::{
    AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite,
    AsyncWriteExt as TokioAsyncWriteExt, ReadBuf,
};
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)

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

pin_project! {
    /// Reader for streaming cached values
    pub struct CacheReader {
        #[pin]
        inner: CacheReaderInner,
        metadata: Arc<CacheMetadata>,
        hasher: Arc<RwLock<Sha256>>,
        bytes_read: u64,
    }
}

enum CacheReaderInner {
    File(tokio::io::BufReader<File>),
    Memory(io::Cursor<Vec<u8>>),
    #[cfg(target_os = "linux")]
    Mmap(MmapReader),
}

#[cfg(target_os = "linux")]
struct MmapReader {
    mmap: memmap2::Mmap,
    position: usize,
}

impl CacheReader {
    /// Create a new file-backed reader
    pub async fn from_file(path: PathBuf, metadata: CacheMetadata) -> Result<Self> {
        let file = match File::open(&path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheError::Io {
                    path,
                    operation: "open cache file for streaming",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        };

        Ok(Self {
            inner: CacheReaderInner::File(tokio::io::BufReader::new(file)),
            metadata: Arc::new(metadata),
            hasher: Arc::new(RwLock::new(Sha256::new())),
            bytes_read: 0,
        })
    }

    /// Create a new memory-backed reader
    pub fn from_memory(data: Vec<u8>, metadata: CacheMetadata) -> Self {
        Self {
            inner: CacheReaderInner::Memory(io::Cursor::new(data)),
            metadata: Arc::new(metadata),
            hasher: Arc::new(RwLock::new(Sha256::new())),
            bytes_read: 0,
        }
    }

    /// Create a memory-mapped reader for zero-copy operations
    #[cfg(target_os = "linux")]
    pub async fn from_mmap(path: PathBuf, metadata: CacheMetadata) -> Result<Self> {
        use std::fs::OpenOptions;

        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheError::Io {
                    path,
                    operation: "open file for memory mapping",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        };

        let mmap = match unsafe { memmap2::MmapOptions::new().map(&file) } {
            Ok(m) => m,
            Err(e) => {
                return Err(CacheError::Io {
                    path,
                    operation: "memory-map cache file",
                    source: e,
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check available memory and file permissions".to_string(),
                    },
                });
            }
        };

        Ok(Self {
            inner: CacheReaderInner::Mmap(MmapReader { mmap, position: 0 }),
            metadata: Arc::new(metadata),
            hasher: Arc::new(RwLock::new(Sha256::new())),
            bytes_read: 0,
        })
    }

    /// Get the metadata for this cached entry
    #[inline]
    pub fn metadata(&self) -> &CacheMetadata {
        &self.metadata
    }

    /// Get the number of bytes read so far
    #[inline]
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Verify the integrity of the data read
    pub fn verify_integrity(&self) -> bool {
        let hasher = self.hasher.read();
        let computed_hash = format!("{:x}", hasher.clone().finalize());
        computed_hash == self.metadata.content_hash
    }
}

impl AsyncRead for CacheReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();

        let result = match this.inner.get_mut() {
            CacheReaderInner::File(reader) => {
                let mut read_buf = ReadBuf::new(buf);
                match Pin::new(reader).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => Poll::Ready(Ok(read_buf.filled().len())),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
                }
            }
            CacheReaderInner::Memory(cursor) => {
                let bytes_to_read = buf
                    .len()
                    .min(cursor.get_ref().len() - cursor.position() as usize);
                if bytes_to_read == 0 {
                    Poll::Ready(Ok(0))
                } else {
                    let start = cursor.position() as usize;
                    let end = start + bytes_to_read;
                    buf[..bytes_to_read].copy_from_slice(&cursor.get_ref()[start..end]);
                    cursor.set_position((start + bytes_to_read) as u64);
                    Poll::Ready(Ok(bytes_to_read))
                }
            }
            #[cfg(target_os = "linux")]
            CacheReaderInner::Mmap(reader) => {
                let remaining = reader.mmap.len() - reader.position;
                if remaining == 0 {
                    return Poll::Ready(Ok(0));
                }

                let to_read = buf.len().min(remaining);
                buf[..to_read]
                    .copy_from_slice(&reader.mmap[reader.position..reader.position + to_read]);
                reader.position += to_read;
                Poll::Ready(Ok(to_read))
            }
        };

        if let Poll::Ready(Ok(n)) = &result {
            *this.bytes_read += *n as u64;
            // Update hash
            this.hasher.write().update(&buf[..*n]);
        }

        result
    }
}

pin_project! {
    /// Writer for streaming values into the cache
    pub struct CacheWriter {
        #[pin]
        file: tokio::io::BufWriter<File>,
        temp_path: PathBuf,
        final_path: PathBuf,
        metadata_path: PathBuf,
        hasher: Sha256,
        bytes_written: u64,
        ttl: Option<Duration>,
        created_at: SystemTime,
    }
}

impl CacheWriter {
    /// Create a new cache writer
    pub async fn new(cache_dir: &PathBuf, key: &str, ttl: Option<Duration>) -> Result<Self> {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        let hash = Self::hash_key(key);
        let (final_path, metadata_path) = Self::get_paths(cache_dir, &hash);

        // Ensure parent directories exist
        if let Some(parent) = final_path.parent() {
            match tokio::fs::create_dir_all(parent).await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: parent.to_path_buf(),
                        operation: "create cache directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: parent.to_path_buf(),
                        },
                    });
                }
            }
        }

        let temp_path = final_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));

        let file = match File::create(&temp_path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheError::Io {
                    path: temp_path.clone(),
                    operation: "create temporary cache file",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: temp_path },
                });
            }
        };

        Ok(Self {
            file: tokio::io::BufWriter::new(file),
            temp_path,
            final_path,
            metadata_path,
            hasher: Sha256::new(),
            bytes_written: 0,
            ttl,
            created_at: SystemTime::now(),
        })
    }

    /// Finalize the write operation
    pub async fn finalize(mut self) -> Result<CacheMetadata> {
        // Flush any buffered data
        match self.file.flush().await {
            Ok(()) => {}
            Err(e) => {
                // Clean up temp file
                let _ = tokio::fs::remove_file(&self.temp_path).await;
                return Err(CacheError::Io {
                    path: self.temp_path.clone(),
                    operation: "flush cache writer",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        }

        // Sync to disk for durability
        match self.file.get_mut().sync_all().await {
            Ok(()) => {}
            Err(e) => {
                let _ = tokio::fs::remove_file(&self.temp_path).await;
                return Err(CacheError::Io {
                    path: self.temp_path.clone(),
                    operation: "sync cache file to disk",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(100),
                    },
                });
            }
        }

        let metadata = CacheMetadata {
            created_at: self.created_at,
            last_accessed: self.created_at,
            expires_at: self.ttl.map(|d| self.created_at + d),
            size_bytes: self.bytes_written,
            access_count: 0,
            content_hash: format!("{:x}", self.hasher.finalize()),
            cache_version: 3, // Version 3 with streaming support
        };

        // Write metadata
        let metadata_bytes = match bincode::serialize(&metadata) {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = tokio::fs::remove_file(&self.temp_path).await;
                return Err(CacheError::Serialization {
                    key: String::new(),
                    operation: crate::cache::errors::SerializationOp::Encode,
                    source: Box::new(e),
                    recovery_hint: RecoveryHint::Manual {
                        instructions: "Check metadata serialization".to_string(),
                    },
                });
            }
        };

        // Ensure metadata directory exists
        if let Some(parent) = self.metadata_path.parent() {
            match tokio::fs::create_dir_all(parent).await {
                Ok(()) => {}
                Err(e) => {
                    let _ = tokio::fs::remove_file(&self.temp_path).await;
                    return Err(CacheError::Io {
                        path: parent.to_path_buf(),
                        operation: "create metadata directory",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: parent.to_path_buf(),
                        },
                    });
                }
            }
        }

        let temp_metadata = self
            .metadata_path
            .with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        match tokio::fs::write(&temp_metadata, &metadata_bytes).await {
            Ok(()) => {}
            Err(e) => {
                let _ = tokio::fs::remove_file(&self.temp_path).await;
                return Err(CacheError::Io {
                    path: temp_metadata.clone(),
                    operation: "write cache metadata",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: temp_metadata,
                    },
                });
            }
        }

        // Atomic rename of both files
        match tokio::fs::rename(&temp_metadata, &self.metadata_path).await {
            Ok(()) => {}
            Err(e) => {
                let _ = tokio::fs::remove_file(&self.temp_path).await;
                let _ = tokio::fs::remove_file(&temp_metadata).await;
                return Err(CacheError::Io {
                    path: self.metadata_path.clone(),
                    operation: "rename metadata file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        match tokio::fs::rename(&self.temp_path, &self.final_path).await {
            Ok(()) => {}
            Err(e) => {
                // Try to clean up metadata since data rename failed
                let _ = tokio::fs::remove_file(&self.metadata_path).await;
                return Err(CacheError::Io {
                    path: self.final_path.clone(),
                    operation: "rename cache data file",
                    source: e,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_millis(10),
                    },
                });
            }
        }

        Ok(metadata)
    }

    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(&3u32.to_le_bytes()); // Version 3
        format!("{:x}", hasher.finalize())
    }

    fn get_paths(cache_dir: &PathBuf, hash: &str) -> (PathBuf, PathBuf) {
        // Use 256-way sharding as specified in Phase 3
        let shard = &hash[..2];

        let data_path = cache_dir.join("objects").join(shard).join(hash);

        let metadata_path = cache_dir
            .join("metadata")
            .join(shard)
            .join(format!("{hash}.meta"));

        (data_path, metadata_path)
    }
}

impl AsyncWrite for CacheWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();

        // Update hash with written data
        this.hasher.update(buf);

        match this.file.poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                *this.bytes_written += n as u64;
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().file.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().file.poll_flush(cx)
    }
}

/// Zero-copy operations for Linux systems
#[cfg(target_os = "linux")]
pub mod zero_copy {
    use super::*;
    use std::os::unix::io::RawFd;

    /// Copy data between file descriptors using sendfile (zero-copy)
    #[allow(dead_code)]
    pub async fn sendfile_copy(from_fd: RawFd, to_fd: RawFd, count: usize) -> io::Result<usize> {
        use libc::{off_t, sendfile};

        let result = unsafe { sendfile(to_fd, from_fd, std::ptr::null_mut::<off_t>(), count) };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }

    /// Copy data using splice for pipe operations (zero-copy)
    #[allow(dead_code)]
    pub async fn splice_copy(from_fd: RawFd, to_fd: RawFd, count: usize) -> io::Result<usize> {
        use libc::{splice, SPLICE_F_MORE, SPLICE_F_MOVE};

        let flags = SPLICE_F_MOVE | SPLICE_F_MORE;
        let result = unsafe {
            splice(
                from_fd,
                std::ptr::null_mut(),
                to_fd,
                std::ptr::null_mut(),
                count,
                flags as std::os::raw::c_uint,
            )
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }
}

/// Vectored I/O operations for scatter-gather
pub mod vectored {
    use super::*;
    use std::io::IoSlice;

    /// Read into multiple buffers (scatter)
    #[allow(dead_code)]
    pub async fn read_vectored<R: AsyncRead + Unpin>(
        reader: &mut R,
        bufs: &mut [IoSlice<'_>],
    ) -> io::Result<usize> {
        let mut total = 0;
        for buf in bufs {
<<<<<<< HEAD
            // IoSlice doesn't implement DerefMut, so we need to work around this
            let slice =
                unsafe { std::slice::from_raw_parts_mut(buf.as_ptr() as *mut u8, buf.len()) };
            let n = reader.read(slice).await?;
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
            let n = reader.read(buf).await?;
=======
            let n = reader.read(&mut **buf).await?;
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
            total += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(total)
    }

    /// Write from multiple buffers (gather)
    #[allow(dead_code)]
    pub async fn write_vectored<W: AsyncWrite + Unpin>(
        writer: &mut W,
        bufs: &[IoSlice<'_>],
    ) -> io::Result<usize> {
        let mut total = 0;
        for buf in bufs {
            let n = writer.write(buf).await?;
            total += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_streaming_write_read() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();

        // Create necessary directories
        tokio::fs::create_dir_all(cache_dir.join("objects"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(cache_dir.join("metadata"))
            .await
            .unwrap();

        // Write data using streaming API
        let mut writer = CacheWriter::new(&cache_dir, "test_key", None).await?;
        writer.write_all(b"Hello, streaming cache!").await.unwrap();
        let metadata = writer.finalize().await?;

        assert_eq!(metadata.size_bytes, 23);

        // Read data back
        let hash = CacheWriter::hash_key("test_key");
        let (data_path, _) = CacheWriter::get_paths(&cache_dir, &hash);

        let mut reader = CacheReader::from_file(data_path, metadata).await?;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).await.unwrap();

        assert_eq!(&buffer, b"Hello, streaming cache!");
        assert!(reader.verify_integrity());

        Ok(())
    }

    #[tokio::test]
    async fn test_large_streaming_copy() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();

        // Create necessary directories
        tokio::fs::create_dir_all(cache_dir.join("objects"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(cache_dir.join("metadata"))
            .await
            .unwrap();

        // Create large test data (10MB)
        let large_data = vec![0x42u8; 10 * 1024 * 1024];

        // Write using streaming
        let mut writer = CacheWriter::new(&cache_dir, "large_key", None).await?;
        writer.write_all(&large_data).await.unwrap();
        let metadata = writer.finalize().await?;

        assert_eq!(metadata.size_bytes, large_data.len() as u64);

        // Read back and verify
        let hash = CacheWriter::hash_key("large_key");
        let (data_path, _) = CacheWriter::get_paths(&cache_dir, &hash);

        let mut reader = CacheReader::from_file(data_path, metadata).await?;
        let mut read_data = Vec::new();
        reader.read_to_end(&mut read_data).await.unwrap();

        assert_eq!(read_data.len(), large_data.len());
        assert!(reader.verify_integrity());

        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_mmap_reader() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.dat");

        // Write test data
        tokio::fs::write(&test_file, b"Memory mapped data")
            .await
            .unwrap();

        let metadata = CacheMetadata {
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: 18,
            access_count: 0,
            content_hash: {
                let mut hasher = Sha256::new();
                hasher.update(b"Memory mapped data");
                format!("{:x}", hasher.finalize())
            },
            cache_version: 3,
        };

        // Read using mmap
        let mut reader = CacheReader::from_mmap(test_file, metadata).await?;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).await.unwrap();

        assert_eq!(&buffer, b"Memory mapped data");
        assert!(reader.verify_integrity());

        Ok(())
    }
}
