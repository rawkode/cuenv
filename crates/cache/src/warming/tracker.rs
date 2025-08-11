//! Access tracking for cache entries

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

/// Access tracking for cache entries
#[derive(Clone)]
pub(crate) struct AccessTracker {
    /// Access counts per key
    pub access_counts: HashMap<String, AccessInfo>,
    /// Last cleanup time
    pub last_cleanup: Instant,
}

#[derive(Clone)]
pub(crate) struct AccessInfo {
    pub count: u64,
    pub last_access: SystemTime,
    pub size: u64,
}

impl AccessTracker {
    pub fn new() -> Self {
        Self {
            access_counts: HashMap::new(),
            last_cleanup: Instant::now(),
        }
    }

    /// Record an access for tracking
    pub fn record_access(&mut self, key: &str, size: u64) {
        let entry = self
            .access_counts
            .entry(key.to_string())
            .or_insert(AccessInfo {
                count: 0,
                last_access: SystemTime::now(),
                size,
            });

        entry.count += 1;
        entry.last_access = SystemTime::now();
        entry.size = size; // Update size in case it changed
    }

    /// Cleanup old entries based on access window
    pub fn cleanup_old(&mut self, access_window: Duration) {
        if self.last_cleanup.elapsed() > Duration::from_secs(3600) {
            let cutoff = SystemTime::now() - access_window;
            self.access_counts
                .retain(|_, info| info.last_access > cutoff);
            self.last_cleanup = Instant::now();
        }
    }

    /// Get candidates that meet minimum access count
    pub fn get_candidates(&self, min_access_count: u64) -> Vec<(String, u64)> {
        self.access_counts
            .iter()
            .filter(|(_, info)| info.count >= min_access_count)
            .map(|(key, info)| (key.clone(), info.count))
            .collect()
    }

    /// Get total size of all tracked entries
    pub fn total_tracked_size(&self) -> u64 {
        self.access_counts.values().map(|info| info.size).sum()
    }

    /// Get candidates sorted by size (largest first) for size-aware warming
    pub fn get_candidates_by_size(&self, max_total_size: u64) -> Vec<(String, u64)> {
        let mut candidates: Vec<_> = self
            .access_counts
            .iter()
            .map(|(key, info)| (key.clone(), info.size, info.count))
            .collect();

        // Sort by size descending, then by count descending
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

        let mut total_size = 0u64;
        let mut result = Vec::new();

        for (key, size, _) in candidates {
            if total_size + size <= max_total_size {
                total_size += size;
                result.push((key, size));
            }
        }

        result
    }
}
