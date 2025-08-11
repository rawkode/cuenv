//! Cache put operations

mod disk;
mod memory;
mod validation;

use crate::errors::Result;
use crate::traits::{CacheKey, CacheMetadata};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use crate::core::types::Cache;
use super::utils::serialize;

impl Cache {
    /// Put a value into the cache
    pub async fn put<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        let data = match serialize(value) {
            Ok(d) => d,
            Err(e) => {
                // Increment error counter for failed serialization
                self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                return Err(e);
            }
        };

        // Validate entry size
        self.validate_entry_size(data.len())?;

        let now = SystemTime::now();

        // Resolve TTL - use provided TTL or fall back to default TTL
        let effective_ttl = ttl.or(self.inner.config.default_ttl);

        // Check entry count limit
        self.check_entry_count_limit(key)?;

        // Fast path for small values (< 256 bytes)
        if data.len() < 256 {
            let metadata = CacheMetadata {
                created_at: now,
                last_accessed: now,
                expires_at: effective_ttl.map(|d| now + d),
                size_bytes: data.len() as u64,
                access_count: 0,
                content_hash: {
                    let mut hasher = Sha256::new();
                    hasher.update(&data);
                    format!("{:x}", hasher.finalize())
                },
                cache_version: self.inner.version,
            };

            let is_replacing_existing = self.inner.memory_cache.contains_key(key)
                || self.inner.fast_path.contains_small(key);

            if self
                .inner
                .fast_path
                .put_small(key.to_string(), data.clone(), metadata)
            {
                self.inner.stats.writes.fetch_add(1, Ordering::Relaxed);
                // Only increment entry count if this is a new entry
                if !is_replacing_existing {
                    self.inner.stats.entry_count.fetch_add(1, Ordering::Relaxed);
                }
                return Ok(());
            }
        }

        let metadata = CacheMetadata {
            created_at: now,
            last_accessed: now,
            expires_at: effective_ttl.map(|d| now + d),
            size_bytes: data.len() as u64,
            access_count: 0,
            content_hash: {
                let mut hasher = Sha256::new();
                hasher.update(&data);
                format!("{:x}", hasher.finalize())
            },
            cache_version: self.inner.version,
        };

        // Handle memory pressure
        self.handle_memory_pressure(data.len()).await;

        // Check capacity
        self.check_capacity(data.len())?;

        // Store in memory cache
        self.store_in_memory(key, data.clone(), metadata.clone())
            .await;

        // Write to disk
        self.write_to_disk(key, &data, &metadata).await?;

        self.inner.stats.writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}