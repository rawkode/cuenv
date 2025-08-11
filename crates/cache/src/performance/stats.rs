//! High-performance statistics counter with cache-line padding

use super::alignment::CacheLineAligned;
use std::sync::atomic::{AtomicU64, Ordering};

/// High-performance statistics counter with cache-line padding
#[repr(C)]
pub struct PerfStats {
    // Each counter gets its own cache line to prevent false sharing
    pub hits: CacheLineAligned<AtomicU64>,
    pub misses: CacheLineAligned<AtomicU64>,
    pub writes: CacheLineAligned<AtomicU64>,
    pub removals: CacheLineAligned<AtomicU64>,
    pub errors: CacheLineAligned<AtomicU64>,
    pub bytes_read: CacheLineAligned<AtomicU64>,
    pub bytes_written: CacheLineAligned<AtomicU64>,
    pub io_operations: CacheLineAligned<AtomicU64>,
}

impl Default for PerfStats {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfStats {
    pub const fn new() -> Self {
        Self {
            hits: CacheLineAligned(AtomicU64::new(0)),
            misses: CacheLineAligned(AtomicU64::new(0)),
            writes: CacheLineAligned(AtomicU64::new(0)),
            removals: CacheLineAligned(AtomicU64::new(0)),
            errors: CacheLineAligned(AtomicU64::new(0)),
            bytes_read: CacheLineAligned(AtomicU64::new(0)),
            bytes_written: CacheLineAligned(AtomicU64::new(0)),
            io_operations: CacheLineAligned(AtomicU64::new(0)),
        }
    }

    #[inline(always)]
    pub fn record_hit(&self) {
        self.hits.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_miss(&self) {
        self.misses.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_write(&self, bytes: u64) {
        self.writes.0.fetch_add(1, Ordering::Relaxed);
        self.bytes_written.0.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_read(&self, bytes: u64) {
        self.bytes_read.0.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_io_op(&self) {
        self.io_operations.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_removal(&self) {
        self.removals.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_error(&self) {
        self.errors.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current statistics as a snapshot
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            hits: self.hits.0.load(Ordering::Relaxed),
            misses: self.misses.0.load(Ordering::Relaxed),
            writes: self.writes.0.load(Ordering::Relaxed),
            removals: self.removals.0.load(Ordering::Relaxed),
            errors: self.errors.0.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.0.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.0.load(Ordering::Relaxed),
            io_operations: self.io_operations.0.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters to zero
    pub fn reset(&self) {
        self.hits.0.store(0, Ordering::Relaxed);
        self.misses.0.store(0, Ordering::Relaxed);
        self.writes.0.store(0, Ordering::Relaxed);
        self.removals.0.store(0, Ordering::Relaxed);
        self.errors.0.store(0, Ordering::Relaxed);
        self.bytes_read.0.store(0, Ordering::Relaxed);
        self.bytes_written.0.store(0, Ordering::Relaxed);
        self.io_operations.0.store(0, Ordering::Relaxed);
    }
}

/// A snapshot of statistics at a point in time
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub removals: u64,
    pub errors: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub io_operations: u64,
}

impl StatsSnapshot {
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
        self.hits + self.misses + self.writes + self.removals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_operations() {
        let stats = PerfStats::new();

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();
        stats.record_write(100);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.hits, 2);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.writes, 1);
        assert_eq!(snapshot.bytes_written, 100);
    }

    #[test]
    fn test_hit_rate() {
        let snapshot = StatsSnapshot {
            hits: 75,
            misses: 25,
            writes: 0,
            removals: 0,
            errors: 0,
            bytes_read: 0,
            bytes_written: 0,
            io_operations: 0,
        };

        assert_eq!(snapshot.hit_rate(), 75.0);
    }
}
