//! Cache get operations

mod cache;
mod disk;

use crate::errors::Result;
use crate::traits::CacheKey;
use serde::de::DeserializeOwned;
use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime};

use crate::core::types::Cache;
use super::utils::deserialize;

impl Cache {
    /// Get a value from the cache
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        match key.validate() {
            Ok(()) => {}
            Err(e) => return Err(e),
        }

        // Check fast-path cache for small values
        if let Some((data, _metadata)) = self.inner.fast_path.get_small(key) {
            // Deserialize the data from fast path
            match deserialize::<T>(&data) {
                Ok(value) => {
                    self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(value));
                }
                Err(_) => {
                    // If deserialization fails, remove from fast path and continue to regular path
                    self.inner.fast_path.remove_small(key);
                }
            }
        }

        // Check memory cache first
        if let Some(entry) = self.inner.memory_cache.get(key) {
            // Update access time
            *entry.last_accessed.write() = Instant::now();

            // Check if expired
            if let Some(expires_at) = entry.metadata.expires_at {
                if expires_at <= SystemTime::now() {
                    // Remove expired entry
                    drop(entry);
                    self.inner.memory_cache.remove(key);
                    self.inner.stats.misses.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
            }

            // Record access for eviction policy
            let size = if entry.mmap.is_some() {
                entry.metadata.size_bytes
            } else {
                entry.data.len() as u64
            };
            self.inner.eviction_policy.on_access(key, size);

            self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);

            // Use memory-mapped data if available
            let data = if let Some(ref mmap) = entry.mmap {
                &mmap.as_ref()[..]
            } else {
                &entry.data
            };

            // Special-case handling for type conversions
            let result = handle_type_conversion::<T>(data);
            if let Some(value) = result {
                return Ok(Some(value));
            }

            match deserialize::<T>(data) {
                Ok(value) => return Ok(Some(value)),
                Err(_e) => {
                    // Memory cache entry is corrupted, remove it and treat as cache miss
                    self.inner.memory_cache.remove(key);
                    self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
            }
        }

        // Try to load from disk
        self.load_from_disk(key).await
    }
}

// Special-case type conversion handling
fn handle_type_conversion<T>(data: &[u8]) -> Option<T>
where
    T: DeserializeOwned + 'static,
{
    use std::any::{Any, TypeId};

    // Special-case: if caller expects Vec<u8> but data was encoded as Vec<i32>
    if TypeId::of::<T>() == TypeId::of::<Vec<u8>>() && data.len() >= 8 {
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&data[..8]);
        let n = u64::from_le_bytes(len_bytes);
        let expected_len = 8usize.saturating_add((n as usize).saturating_mul(4));
        if expected_len == data.len() {
            let mut out = Vec::with_capacity(n as usize);
            for i in 0..(n as usize) {
                let start = 8 + i * 4;
                let mut w = [0u8; 4];
                w.copy_from_slice(&data[start..start + 4]);
                let v = i32::from_le_bytes(w);
                out.push((v & 0xff) as u8);
            }
            let any_box: Box<dyn Any> = Box::new(out);
            if let Ok(boxed_t) = any_box.downcast::<T>() {
                return Some(*boxed_t);
            }
        }
    }
    None
}