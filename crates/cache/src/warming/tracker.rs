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
}
