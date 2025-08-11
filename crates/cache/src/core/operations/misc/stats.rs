//! Cache statistics operations

use crate::core::types::Cache;
use crate::errors::Result;
use crate::traits::CacheStatistics;
use std::sync::atomic::Ordering;

impl Cache {
    /// Get cache statistics
    pub async fn statistics(&self) -> Result<CacheStatistics> {
        let entry_count = self.inner.stats.entry_count.load(Ordering::Relaxed);

        Ok(CacheStatistics {
            hits: self.inner.stats.hits.load(Ordering::Relaxed),
            misses: self.inner.stats.misses.load(Ordering::Relaxed),
            writes: self.inner.stats.writes.load(Ordering::Relaxed),
            removals: self.inner.stats.removals.load(Ordering::Relaxed),
            errors: self.inner.stats.errors.load(Ordering::Relaxed),
            entry_count,
            total_bytes: self.inner.stats.total_bytes.load(Ordering::Relaxed),
            max_bytes: self.inner.config.max_size_bytes,
            expired_cleanups: self.inner.stats.expired_cleanups.load(Ordering::Relaxed),
            stats_since: self.inner.stats.stats_since,
            compression_enabled: self.inner.config.compression_enabled,
            compression_ratio: 1.0, // TODO: Track actual compression ratio
            wal_recoveries: 0,      // TODO: Track WAL recoveries
            checksum_failures: 0,   // TODO: Track checksum failures
        })
    }
}