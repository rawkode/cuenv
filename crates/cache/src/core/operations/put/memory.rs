//! Memory management for put operations

use crate::core::internal::InMemoryEntry;
use crate::core::types::Cache;
use crate::traits::CacheMetadata;
use parking_lot::RwLock;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

impl Cache {
    /// Store entry in memory cache if there's capacity
    pub(super) async fn store_in_memory(
        &self,
        key: &str,
        data: Vec<u8>,
        metadata: CacheMetadata,
    ) -> bool {
        let can_store_in_memory = match self.inner.config.max_memory_size {
            Some(max) => {
                let current = self.inner.stats.total_bytes.load(Ordering::Relaxed);
                current.saturating_add(data.len() as u64) <= max
            }
            None => true,
        };

        if can_store_in_memory {
            let entry = Arc::new(InMemoryEntry {
                mmap: None, // Will be set on next read
                data: data.clone(),
                metadata,
                last_accessed: RwLock::new(Instant::now()),
            });

            // If we're replacing an existing entry, subtract its size
            let is_new_entry =
                if let Some(old_entry) = self.inner.memory_cache.insert(key.to_string(), entry) {
                    self.inner
                        .stats
                        .total_bytes
                        .fetch_sub(old_entry.data.len() as u64, Ordering::Relaxed);
                    false // Replacing existing entry
                } else {
                    true // New entry
                };

            self.inner
                .stats
                .total_bytes
                .fetch_add(data.len() as u64, Ordering::Relaxed);

            // Update entry count if this is a new entry
            if is_new_entry {
                self.inner.stats.entry_count.fetch_add(1, Ordering::Relaxed);
            }

            // Record insertion with eviction policy only when we added to in-memory cache
            self.inner.eviction_policy.on_insert(key, data.len() as u64);
            true
        } else {
            // Memory is at or above limit. Proactively evict one victim
            let mut victim = self.inner.eviction_policy.next_eviction();

            if victim.is_none() {
                // Fallback: choose the least-recently-accessed in-memory entry.
                let mut oldest_key: Option<String> = None;
                let mut oldest_instant = Instant::now();
                for item in self.inner.memory_cache.iter() {
                    let last = *item.value().last_accessed.read();
                    if oldest_key.is_none() || last < oldest_instant {
                        oldest_instant = last;
                        oldest_key = Some(item.key().clone());
                    }
                }
                victim = oldest_key;
            }

            if let Some(v) = victim {
                // Best-effort eviction; ignore errors to avoid failing this put.
                let _ = self.remove(&v).await;
            }
            false
        }
    }

    /// Check and handle memory pressure before adding new entry
    pub(super) async fn handle_memory_pressure(&self, data_len: usize) {
        // Check if we need to evict due to memory limits before adding new entry
        if let Some(max_memory) = self.inner.config.max_memory_size {
            let current_memory = self.inner.stats.total_bytes.load(Ordering::Relaxed);
            let new_total = current_memory.saturating_add(data_len as u64);

            if new_total > max_memory {
                // Run eviction to make space
                match self.evict_entries().await {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::warn!("Failed to evict entries: {}", e);
                    }
                }
            }
        }

        // Check memory pressure and disk quota
        if !self.inner.memory_manager.can_allocate(data_len as u64) {
            // Run eviction
            match self.evict_entries().await {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!("Failed to evict entries: {}", e);
                }
            }
        }

        match self.inner.memory_manager.check_disk_quota(data_len as u64) {
            Ok(_) => {}
            Err(e) => {
                // Try eviction first
                match self.evict_entries().await {
                    Ok(()) => {
                        // Retry quota check
                        match self.inner.memory_manager.check_disk_quota(data_len as u64) {
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!("Disk quota exceeded after eviction: {}", e);
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Disk quota exceeded and eviction failed: {}", e);
                    }
                }
            }
        }
    }
}
