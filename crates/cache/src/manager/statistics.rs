//! Cache statistics tracking and reporting

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// Statistics for cache operations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub errors: u64,
    pub lock_contentions: u64,
    pub total_bytes_saved: u64,
    pub last_cleanup: Option<SystemTime>,
}

impl CacheStatistics {
    /// Create new statistics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cache hit
    pub fn record_hit(&mut self) {
        self.hits += 1;
    }

    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.misses += 1;
    }

    /// Record a write operation
    pub fn record_write(&mut self) {
        self.writes += 1;
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    /// Record lock contention
    pub fn record_lock_contention(&mut self) {
        self.lock_contentions += 1;
    }

    /// Record bytes saved
    pub fn record_bytes_saved(&mut self, bytes: u64) {
        self.total_bytes_saved += bytes;
    }

    /// Update last cleanup time
    pub fn record_cleanup(&mut self) {
        self.last_cleanup = Some(SystemTime::now());
    }

    /// Calculate hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    /// Get total operations count
    pub fn total_operations(&self) -> u64 {
        self.hits + self.misses + self.writes
    }
}

/// Thread-safe statistics container
pub struct StatsContainer {
    stats: Arc<RwLock<CacheStatistics>>,
}

impl StatsContainer {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(CacheStatistics::new())),
        }
    }

    pub fn record_hit(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.record_hit();
        }
    }

    pub fn record_miss(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.record_miss();
        }
    }

    pub fn record_write(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.record_write();
        }
    }

    pub fn record_error(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.record_error();
        }
    }

    pub fn record_cleanup(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.record_cleanup();
        }
    }

    pub fn get_snapshot(&self) -> CacheStatistics {
        self.stats.read().unwrap().clone()
    }

    pub fn get_stats_ref(&self) -> Arc<RwLock<CacheStatistics>> {
        Arc::clone(&self.stats)
    }
}

impl Default for StatsContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_tracking() {
        let mut stats = CacheStatistics::new();

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();
        stats.record_write();

        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.writes, 1);
        assert_eq!(stats.hit_rate(), 66.66666666666666);
    }

    #[test]
    fn test_stats_container() {
        let container = StatsContainer::new();

        container.record_hit();
        container.record_miss();
        container.record_write();

        let snapshot = container.get_snapshot();
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.writes, 1);
    }
}
