//! Hit rate analysis for cache effectiveness

mod rolling_window;
mod types;

use self::rolling_window::RollingWindow;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use types::{HitRateReport, OperationStats, PatternStats};

/// Hit rate analyzer for cache effectiveness
pub struct HitRateAnalyzer {
    /// Hit rates by time window
    time_windows: RwLock<TimeWindowStats>,
    /// Hit rates by key pattern
    key_patterns: RwLock<HashMap<String, HitRateStats>>,
    /// Hit rates by operation type
    operation_types: RwLock<HashMap<String, HitRateStats>>,
}

struct TimeWindowStats {
    /// 1-minute window
    one_minute: RollingWindow,
    /// 5-minute window
    five_minutes: RollingWindow,
    /// 1-hour window
    one_hour: RollingWindow,
    /// 24-hour window
    one_day: RollingWindow,
}

struct HitRateStats {
    hits: AtomicU64,
    misses: AtomicU64,
    last_access: RwLock<Instant>,
}

impl HitRateAnalyzer {
    pub fn new() -> Self {
        Self {
            time_windows: RwLock::new(TimeWindowStats {
                one_minute: RollingWindow::new(Duration::from_secs(60)),
                five_minutes: RollingWindow::new(Duration::from_secs(300)),
                one_hour: RollingWindow::new(Duration::from_secs(3600)),
                one_day: RollingWindow::new(Duration::from_secs(86400)),
            }),
            key_patterns: RwLock::new(HashMap::new()),
            operation_types: RwLock::new(HashMap::new()),
        }
    }

    pub fn record_hit(&self, key: &str, operation: &str) {
        // Update time windows
        {
            let windows = self.time_windows.read();
            windows.one_minute.record_hit();
            windows.five_minutes.record_hit();
            windows.one_hour.record_hit();
            windows.one_day.record_hit();
        }

        // Update key pattern stats
        if let Some(pattern) = Self::extract_pattern(key) {
            let mut patterns = self.key_patterns.write();
            let stats = patterns.entry(pattern).or_insert_with(|| HitRateStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                last_access: RwLock::new(Instant::now()),
            });
            stats.hits.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }

        // Update operation type stats
        {
            let mut op_types = self.operation_types.write();
            let stats = op_types
                .entry(operation.to_string())
                .or_insert_with(|| HitRateStats {
                    hits: AtomicU64::new(0),
                    misses: AtomicU64::new(0),
                    last_access: RwLock::new(Instant::now()),
                });
            stats.hits.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }
    }

    pub fn record_miss(&self, key: &str, operation: &str) {
        // Update time windows
        {
            let windows = self.time_windows.read();
            windows.one_minute.record_miss();
            windows.five_minutes.record_miss();
            windows.one_hour.record_miss();
            windows.one_day.record_miss();
        }

        // Update key pattern stats
        if let Some(pattern) = Self::extract_pattern(key) {
            let mut patterns = self.key_patterns.write();
            let stats = patterns.entry(pattern).or_insert_with(|| HitRateStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                last_access: RwLock::new(Instant::now()),
            });
            stats.misses.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }

        // Update operation type stats
        {
            let mut op_types = self.operation_types.write();
            let stats = op_types
                .entry(operation.to_string())
                .or_insert_with(|| HitRateStats {
                    hits: AtomicU64::new(0),
                    misses: AtomicU64::new(0),
                    last_access: RwLock::new(Instant::now()),
                });
            stats.misses.fetch_add(1, Ordering::Relaxed);
            *stats.last_access.write() = Instant::now();
        }
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let windows = self.time_windows.read();
        windows.one_hour.hit_rate()
    }

    pub fn generate_report(&self) -> HitRateReport {
        let windows = self.time_windows.read();

        let mut key_patterns = Vec::new();
        for (pattern, stats) in self.key_patterns.read().iter() {
            let hits = stats.hits.load(Ordering::Relaxed);
            let misses = stats.misses.load(Ordering::Relaxed);
            let total = hits + misses;
            if total > 0 {
                key_patterns.push(PatternStats {
                    pattern: pattern.clone(),
                    hit_rate: hits as f64 / total as f64,
                    total_accesses: total,
                });
            }
        }
        key_patterns.sort_by(|a, b| b.total_accesses.cmp(&a.total_accesses));

        let mut operation_types = Vec::new();
        for (op_type, stats) in self.operation_types.read().iter() {
            let hits = stats.hits.load(Ordering::Relaxed);
            let misses = stats.misses.load(Ordering::Relaxed);
            let total = hits + misses;
            if total > 0 {
                operation_types.push(OperationStats {
                    operation: op_type.clone(),
                    hit_rate: hits as f64 / total as f64,
                    total_calls: total,
                });
            }
        }
        operation_types.sort_by(|a, b| b.total_calls.cmp(&a.total_calls));

        HitRateReport {
            one_minute: windows.one_minute.hit_rate(),
            five_minutes: windows.five_minutes.hit_rate(),
            one_hour: windows.one_hour.hit_rate(),
            one_day: windows.one_day.hit_rate(),
            key_patterns,
            operation_types,
        }
    }

    fn extract_pattern(key: &str) -> Option<String> {
        if let Some(colon_pos) = key.find(':') {
            Some(format!("{}:*", &key[..colon_pos]))
        } else {
            key.find('/')
                .map(|slash_pos| format!("{}/*", &key[..slash_pos]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_extraction() {
        assert_eq!(
            HitRateAnalyzer::extract_pattern("user:123"),
            Some("user:*".to_string())
        );
        assert_eq!(
            HitRateAnalyzer::extract_pattern("path/to/file"),
            Some("path/*".to_string())
        );
        assert_eq!(HitRateAnalyzer::extract_pattern("simple_key"), None);
    }
}
