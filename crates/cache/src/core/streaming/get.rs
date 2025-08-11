//! Streaming get operations

use crate::core::types::Cache;
use crate::errors::{CacheError, RecoveryHint, Result};
use futures::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl Cache {
    pub fn get_stream<'a, W>(
        &'a self,
        key: &'a str,
        writer: W,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>>> + Send + 'a>>
    where
        W: AsyncWrite + Send + 'a,
    {
        Box::pin(async move {
            let reader = match self.get_reader(key).await {
                Ok(Some(r)) => r,
                Ok(None) => return Ok(None),
                Err(e) => return Err(e),
            };

            let _expected_size = reader.metadata().size_bytes;

            // High-performance streaming copy
            // Note: Zero-copy implementation would be added here for Linux systems
            // using sendfile/splice system calls for optimal performance

            // Standard async copy
            let mut reader = reader;
            const BUFFER_SIZE: usize = 64 * 1024; // 64KB buffer
            let mut buffer = vec![0u8; BUFFER_SIZE];
            let mut total_bytes = 0u64;

            tokio::pin!(writer);

            loop {
                let n = match reader.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => n,
                    Err(e) => {
                        return Err(CacheError::Io {
                            path: PathBuf::from(key),
                            operation: "read from cache stream",
                            source: std::io::Error::other(e),
                            recovery_hint: RecoveryHint::Retry {
                                after: Duration::from_millis(100),
                            },
                        });
                    }
                };

                match writer.write_all(&buffer[..n]).await {
                    Ok(()) => {}
                    Err(e) => {
                        return Err(CacheError::Io {
                            path: PathBuf::from(key),
                            operation: "write to output stream",
                            source: std::io::Error::other(e),
                            recovery_hint: RecoveryHint::Retry {
                                after: Duration::from_millis(100),
                            },
                        });
                    }
                }

                total_bytes += n as u64;
            }

            match writer.flush().await {
                Ok(()) => {}
                Err(e) => {
                    return Err(CacheError::Io {
                        path: PathBuf::from(key),
                        operation: "flush output stream",
                        source: std::io::Error::other(e),
                        recovery_hint: RecoveryHint::Retry {
                            after: Duration::from_millis(10),
                        },
                    });
                }
            }

            self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);
            Ok(Some(total_bytes))
        })
    }
}