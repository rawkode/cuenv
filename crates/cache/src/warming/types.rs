//! Types and configuration for cache warming

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Cache warming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmingConfig {
    /// How often to run warming
    pub warming_interval: Duration,
    /// Maximum entries to warm per cycle
    pub max_entries_per_cycle: usize,
    /// Minimum access count to be eligible for warming
    pub min_access_count: u64,
    /// Time window for access tracking
    pub access_window: Duration,
    /// Enable predictive warming based on patterns
    pub predictive_warming: bool,
    /// Maximum total size to warm per cycle (0 = unlimited)
    pub max_warming_size: u64,
}

impl Default for WarmingConfig {
    fn default() -> Self {
        Self {
            warming_interval: Duration::from_secs(300), // 5 minutes
            max_entries_per_cycle: 1000,
            min_access_count: 5,
            access_window: Duration::from_secs(3600), // 1 hour
            predictive_warming: true,
            max_warming_size: 0, // Unlimited by default
        }
    }
}

/// Warming statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmingStats {
    pub total_warmed: u64,
    pub warming_cycles: u64,
    pub last_warming: Option<SystemTime>,
    pub patterns_learned: u64,
    pub predictive_hits: u64,
}
