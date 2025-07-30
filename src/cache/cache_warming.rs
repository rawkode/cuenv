//! Cache warming and preloading functionality
//!
//! Provides background cache warming to preload frequently accessed entries.

use crate::cache::errors::{CacheError, RecoveryHint, Result};
use crate::cache::traits::Cache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::time::interval;

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
}

impl Default for WarmingConfig {
    fn default() -> Self {
        Self {
            warming_interval: Duration::from_secs(300), // 5 minutes
            max_entries_per_cycle: 1000,
            min_access_count: 5,
            access_window: Duration::from_secs(3600), // 1 hour
            predictive_warming: true,
        }
    }
}

/// Cache warming engine
pub struct CacheWarmer<C: Cache> {
    /// Cache instance
    cache: C,
    /// Configuration
    config: WarmingConfig,
    /// Access tracking
    access_tracker: Arc<RwLock<AccessTracker>>,
    /// Warming patterns
    patterns: Arc<RwLock<WarmingPatterns>>,
    /// Shutdown flag
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

/// Access tracking for cache entries
struct AccessTracker {
    /// Access counts per key
    access_counts: HashMap<String, AccessInfo>,
    /// Last cleanup time
    last_cleanup: Instant,
}

#[derive(Clone)]
struct AccessInfo {
    count: u64,
    last_access: SystemTime,
    size: u64,
}

/// Warming patterns for predictive loading
struct WarmingPatterns {
    /// Sequential access patterns
    sequential_patterns: HashMap<String, Vec<String>>,
    /// Time-based patterns (hour of day -> keys)
    temporal_patterns: HashMap<u8, HashSet<String>>,
    /// Related keys that are often accessed together
    related_keys: HashMap<String, HashSet<String>>,
}

impl<C: Cache + Clone + Send + Sync + 'static> CacheWarmer<C> {
    pub fn new(cache: C, config: WarmingConfig) -> Self {
        Self {
            cache,
            config,
            access_tracker: Arc::new(RwLock::new(AccessTracker {
                access_counts: HashMap::new(),
                last_cleanup: Instant::now(),
            })),
            patterns: Arc::new(RwLock::new(WarmingPatterns {
                sequential_patterns: HashMap::new(),
                temporal_patterns: HashMap::new(),
                related_keys: HashMap::new(),
            })),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Record an access for tracking
    pub fn record_access(&self, key: &str, size: u64) {
        let mut tracker = self.access_tracker.write();

        let entry = tracker
            .access_counts
            .entry(key.to_string())
            .or_insert(AccessInfo {
                count: 0,
                last_access: SystemTime::now(),
                size,
            });

        entry.count += 1;
        entry.last_access = SystemTime::now();

        // Cleanup old entries periodically
        if tracker.last_cleanup.elapsed() > Duration::from_secs(3600) {
            let cutoff = SystemTime::now() - self.config.access_window;
            tracker
                .access_counts
                .retain(|_, info| info.last_access > cutoff);
            tracker.last_cleanup = Instant::now();
        }
    }

    /// Start the warming engine
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let mut interval = interval(self.config.warming_interval);

        loop {
            interval.tick().await;

            if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }

            match self.warm_cache().await {
                Ok(warmed) => {
                    if warmed > 0 {
                        tracing::info!("Warmed {} cache entries", warmed);
                    }
                }
                Err(e) => {
                    tracing::error!("Cache warming failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Perform cache warming
    async fn warm_cache(&self) -> Result<usize> {
        // Get candidates for warming
        let candidates = self.get_warming_candidates();

        if candidates.is_empty() {
            return Ok(0);
        }

        let mut warmed = 0;

        for (key, _) in candidates.iter().take(self.config.max_entries_per_cycle) {
            // Check if already in cache
            match self.cache.contains(key).await {
                Ok(true) => continue, // Already cached
                Ok(false) => {
                    // Try to warm this entry
                    match self.warm_entry(key).await {
                        Ok(()) => warmed += 1,
                        Err(e) => {
                            tracing::debug!("Failed to warm {}: {}", key, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to check cache for {}: {}", key, e);
                }
            }
        }

        Ok(warmed)
    }

    /// Get candidates for warming based on access patterns
    fn get_warming_candidates(&self) -> Vec<(String, u64)> {
        let tracker = self.access_tracker.read();

        let mut candidates: Vec<(String, u64)> = tracker
            .access_counts
            .iter()
            .filter(|(_, info)| info.count >= self.config.min_access_count)
            .map(|(key, info)| (key.clone(), info.count))
            .collect();

        // Sort by access count (descending)
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        // Add predictive candidates if enabled
        if self.config.predictive_warming {
            self.add_predictive_candidates(&mut candidates);
        }

        candidates
    }

    /// Add predictive warming candidates
    fn add_predictive_candidates(&self, candidates: &mut Vec<(String, u64)>) {
        let patterns = self.patterns.read();

        // Add temporal patterns for current hour
        let current_hour = {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            ((now.as_secs() / 3600) % 24) as u8
        };

        if let Some(hourly_keys) = patterns.temporal_patterns.get(&current_hour) {
            for key in hourly_keys {
                if !candidates.iter().any(|(k, _)| k == key) {
                    candidates.push((key.clone(), self.config.min_access_count));
                }
            }
        }

        // Add related keys for already-selected candidates
        let selected_keys: HashSet<_> = candidates.iter().map(|(k, _)| k.clone()).collect();

        for (key, _) in &selected_keys {
            if let Some(related) = patterns.related_keys.get(key) {
                for related_key in related {
                    if !candidates.iter().any(|(k, _)| k == related_key) {
                        candidates.push((related_key.clone(), self.config.min_access_count));
                    }
                }
            }
        }
    }

    /// Warm a single cache entry
    async fn warm_entry(&self, key: &str) -> Result<()> {
        // This is a generic implementation that just checks if the key exists
        // In practice, you'd load the actual data from the source
        match self.cache.contains(key).await {
            Ok(_) => Ok(()),
            Err(e) => Err(CacheError::Configuration {
                message: format!("Failed to warm cache entry: {}", e),
                recovery_hint: RecoveryHint::Ignore,
            }),
        }
    }

    /// Learn access pattern
    pub fn learn_pattern(&self, keys: &[String]) {
        if keys.len() < 2 {
            return;
        }

        let mut patterns = self.patterns.write();

        // Learn sequential patterns
        for window in keys.windows(2) {
            let pattern_key = window[0].clone();
            patterns
                .sequential_patterns
                .entry(pattern_key)
                .or_insert_with(Vec::new)
                .push(window[1].clone());
        }

        // Learn temporal patterns
        let current_hour = {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            ((now.as_secs() / 3600) % 24) as u8
        };

        for key in keys {
            patterns
                .temporal_patterns
                .entry(current_hour)
                .or_insert_with(HashSet::new)
                .insert(key.clone());
        }

        // Learn related keys (accessed in same batch)
        if keys.len() > 1 {
            for i in 0..keys.len() {
                for j in 0..keys.len() {
                    if i != j {
                        patterns
                            .related_keys
                            .entry(keys[i].clone())
                            .or_insert_with(HashSet::new)
                            .insert(keys[j].clone());
                    }
                }
            }
        }
    }

    /// Shutdown the warmer
    pub fn shutdown(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Release);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheBuilder;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_access_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheBuilder::new(temp_dir.path())
            .build_async()
            .await
            .unwrap();

        let warmer = Arc::new(CacheWarmer::new(
            cache,
            WarmingConfig {
                min_access_count: 2,
                ..Default::default()
            },
        ));

        // Record multiple accesses
        warmer.record_access("key1", 100);
        warmer.record_access("key1", 100);
        warmer.record_access("key2", 200);

        // Check candidates
        let candidates = warmer.get_warming_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "key1");
        assert_eq!(candidates[0].1, 2);
    }

    #[tokio::test]
    async fn test_pattern_learning() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheBuilder::new(temp_dir.path())
            .build_async()
            .await
            .unwrap();

        let warmer = CacheWarmer::new(cache, WarmingConfig::default());

        // Learn a sequential pattern
        warmer.learn_pattern(&[
            "user_profile".to_string(),
            "user_settings".to_string(),
            "user_preferences".to_string(),
        ]);

        let patterns = warmer.patterns.read();
        assert!(patterns.sequential_patterns.contains_key("user_profile"));
        assert!(patterns.related_keys.contains_key("user_profile"));
    }
}
