//! Streaming write operations

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::streaming::CacheWriter;
use crate::traits::CacheKey;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use parking_lot::RwLock;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::internal::InMemoryEntry;
use crate::core::operations::utils::mmap_file;
use crate::core::paths::{hash_key, object_path};
use crate::core::types::Cache;

impl Cache {
    pub fn get_writer<'a>(
        &'a self,
        key: &'a str,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<CacheWriter>> + Send + 'a>> {
        Box::pin(async move {
            match key.validate() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            // Check capacity before creating writer
            if self.inner.config.max_size_bytes > 0 {
                let current_size = self.inner.stats.total_bytes.load(Ordering::Relaxed);
                if current_size >= self.inner.config.max_size_bytes {
                    return Err(CacheError::CapacityExceeded {
                        requested_bytes: 0,
                        available_bytes: 0,
                        recovery_hint: RecoveryHint::IncreaseCapacity {
                            suggested_bytes: self.inner.config.max_size_bytes * 2,
                        },
                    });
                }
            }

            match CacheWriter::new(&self.inner.base_dir, key, ttl).await {
                Ok(writer) => Ok(writer),
                Err(e) => Err(e),
            }
        })
    }

    pub fn put_stream<'a, R>(
        &'a self,
        key: &'a str,
        reader: R,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>
    where
        R: AsyncRead + Send + 'a,
    {
        Box::pin(async move {
            let mut writer = match self.get_writer(key, ttl).await {
                Ok(w) => w,
                Err(e) => return Err(e),
            };

            // High-performance streaming copy
            const BUFFER_SIZE: usize = 64 * 1024; // 64KB buffer
            let mut buffer = vec![0u8; BUFFER_SIZE];
            let mut total_bytes = 0u64;

            tokio::pin!(reader);

            loop {
                let n = match reader.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => n,
                    Err(e) => {
                        return Err(CacheError::Io {
                            path: PathBuf::from(key),
                            operation: "read from stream",
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
                            operation: "write to cache stream",
                            source: std::io::Error::other(e),
                            recovery_hint: RecoveryHint::Retry {
                                after: Duration::from_millis(100),
                            },
                        });
                    }
                }

                total_bytes += n as u64;
            }

            // Finalize the write
            let metadata = match writer.finalize().await {
                Ok(m) => m,
                Err(e) => return Err(e),
            };

            // Update statistics
            self.inner.stats.writes.fetch_add(1, Ordering::Relaxed);
            self.inner
                .stats
                .total_bytes
                .fetch_add(total_bytes, Ordering::Relaxed);

            // Add to memory cache for hot access
            let _hash = hash_key(&self.inner, key);
            let data_path = object_path(&self.inner, key);

            // Try to memory-map for future reads
            let mmap_option = mmap_file(&data_path).ok().map(Arc::new);

            let entry = Arc::new(InMemoryEntry {
                mmap: mmap_option,
                data: Vec::new(), // Empty for streamed entries
                metadata,
                last_accessed: RwLock::new(Instant::now()),
            });

            self.inner.memory_cache.insert(key.to_string(), entry);

            Ok(total_bytes)
        })
    }

}