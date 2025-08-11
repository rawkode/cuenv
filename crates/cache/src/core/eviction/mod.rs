//! Cache eviction logic

use crate::errors::Result;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::types::Cache;

impl Cache {
    /// Evict entries based on the configured eviction policy
    pub(crate) async fn evict_entries(&self) -> Result<()> {
        let mut evicted_count = 0;

        // Use cache's actual memory usage, not the eviction policy's tracking
        let max_memory = match self.inner.config.max_memory_size {
            Some(max) => max,
            None => return Ok(()), // No memory limit configured
        };

        let target_memory = max_memory * 8 / 10; // Target 80% usage

        loop {
            // Check current cache memory usage
            let current_memory = self.inner.stats.total_bytes.load(Ordering::Relaxed);

            // Check if we need to evict more
            if current_memory <= target_memory {
                break;
            }

            // Get next key to evict
            let mut key_to_evict = self.inner.eviction_policy.next_eviction();

            // Fallback: if the policy can't provide a key (e.g., under contention),
            // choose the least-recently-accessed in-memory entry to ensure forward progress.
            if key_to_evict.is_none() {
                let mut oldest_key: Option<String> = None;
                let mut oldest_instant = Instant::now();
                for item in self.inner.memory_cache.iter() {
                    let last = *item.value().last_accessed.read();
                    if oldest_key.is_none() || last < oldest_instant {
                        oldest_instant = last;
                        oldest_key = Some(item.key().clone());
                    }
                }
                key_to_evict = oldest_key;
            }

            let key_to_evict = match key_to_evict {
                Some(key) => key,
                None => break, // No more entries to evict
            };

            // Remove the entry
            match self.remove(&key_to_evict).await {
                Ok(removed) => {
                    if removed {
                        evicted_count += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to evict {}: {}", key_to_evict, e);
                }
            }

            // Limit evictions per call to prevent blocking too long
            if evicted_count >= 100 {
                break;
            }
        }

        if evicted_count > 0 {
            tracing::info!("Evicted {} entries to free memory", evicted_count);
        }

        Ok(())
    }
}
