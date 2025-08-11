//! Put operation validation utilities

use crate::errors::{CacheError, RecoveryHint, Result};
use std::sync::atomic::Ordering;

use crate::core::types::Cache;

impl Cache {
    /// Validate entry size against configured limits
    pub(super) fn validate_entry_size(&self, data_len: usize) -> Result<()> {
        let max_entry_size = self.inner.config.max_size_bytes as usize;
        if max_entry_size > 0 && data_len > max_entry_size {
            self.inner.stats.errors.fetch_add(1, Ordering::Relaxed);
            return Err(CacheError::CapacityExceeded {
                requested_bytes: data_len as u64,
                available_bytes: max_entry_size as u64,
                recovery_hint: RecoveryHint::Manual {
                    instructions: format!(
                        "Entry size {} bytes exceeds maximum of {} bytes",
                        data_len, max_entry_size
                    ),
                },
            });
        }
        Ok(())
    }

    /// Check entry count limit
    pub(super) fn check_entry_count_limit(&self, key: &str) -> Result<()> {
        let current_entry_count = self.inner.stats.entry_count.load(Ordering::Relaxed);
        let is_replacing_existing =
            self.inner.memory_cache.contains_key(key) || self.inner.fast_path.contains_small(key);

        if !is_replacing_existing
            && self.inner.config.max_entries > 0
            && current_entry_count >= self.inner.config.max_entries
        {
            return Err(CacheError::CapacityExceeded {
                requested_bytes: 0,
                available_bytes: 0,
                recovery_hint: RecoveryHint::Manual {
                    instructions: format!(
                        "Cache has reached maximum entry limit of {}. Consider increasing max_entries or clearing old entries.",
                        self.inner.config.max_entries
                    ),
                },
            });
        }
        Ok(())
    }

    /// Check capacity against max_size_bytes
    pub(super) fn check_capacity(&self, data_len: usize) -> Result<()> {
        let new_total = self
            .inner
            .stats
            .total_bytes
            .load(Ordering::Relaxed)
            .saturating_add(data_len as u64);

        if self.inner.config.max_size_bytes > 0 && new_total > self.inner.config.max_size_bytes {
            return Err(CacheError::CapacityExceeded {
                requested_bytes: data_len as u64,
                available_bytes: self
                    .inner
                    .config
                    .max_size_bytes
                    .saturating_sub(self.inner.stats.total_bytes.load(Ordering::Relaxed)),
                recovery_hint: RecoveryHint::IncreaseCapacity {
                    suggested_bytes: new_total,
                },
            });
        }
        Ok(())
    }
}
