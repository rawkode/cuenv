//! Cache reader implementations for streaming operations
//!
//! Provides zero-copy and memory-efficient readers for cached values,
//! supporting file-based, memory-backed, and memory-mapped operations.

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::CacheMetadata;
use futures::io::AsyncRead;
use parking_lot::RwLock;
use pin_project_lite::pin_project;
use sha2::{Digest, Sha256};
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncRead as TokioAsyncRead, ReadBuf};

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
        // Clone the hasher state to avoid consuming the original
        let hasher_guard = self.hasher.read();
        let hasher_clone = hasher_guard.clone();
        drop(hasher_guard); // Release the lock early

        let computed_hash = format!("{:x}", hasher_clone.finalize());
        let expected_hash = &self.metadata.content_hash;
        computed_hash == *expected_hash
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
