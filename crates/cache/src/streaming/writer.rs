//! Cache writer implementation for streaming operations
//!
//! Provides a streaming writer that efficiently writes data directly to cache
//! storage without buffering the entire value in memory.

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::{CacheKey, CacheMetadata};
use futures::io::AsyncWrite;
use pin_project_lite::pin_project;
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};
use tokio::fs::File;
use tokio::io::{AsyncWrite as TokioAsyncWrite, AsyncWriteExt as TokioAsyncWriteExt};

use super::{finalization, path_utils};

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
    pub async fn new(cache_dir: &Path, key: &str, ttl: Option<Duration>) -> Result<Self> {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        let hash = path_utils::hash_key(key);
        let (final_path, metadata_path) = path_utils::get_paths(cache_dir, &hash);

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

        // Perform atomic finalization
        finalization::finalize_cache_files(
            &self.temp_path,
            &self.final_path,
            &self.metadata_path,
            &metadata,
        )
        .await?;

        Ok(metadata)
    }

    /// Hash a cache key with version information
    pub fn hash_key(key: &str) -> String {
        path_utils::hash_key(key)
    }

    /// Get the storage paths for a given hash
    pub fn get_paths(cache_dir: &Path, hash: &str) -> (PathBuf, PathBuf) {
        path_utils::get_paths(cache_dir, hash)
    }
}

impl AsyncWrite for CacheWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();

        match this.file.poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                // Update hash only with bytes actually written
                this.hasher.update(&buf[..n]);
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
