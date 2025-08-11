//! Core cache warming engine

use super::candidates::CandidateWarmer;
use super::patterns::WarmingPatterns;
use super::tracker::AccessTracker;
use super::types::WarmingConfig;
use crate::errors::Result;
use crate::traits::Cache;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::time::interval;

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

impl<C: Cache + Clone + Send + Sync + 'static> CacheWarmer<C> {
    pub fn new(cache: C, config: WarmingConfig) -> Self {
        Self {
            cache,
            config,
            access_tracker: Arc::new(RwLock::new(AccessTracker::new())),
            patterns: Arc::new(RwLock::new(WarmingPatterns::new())),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Record an access for tracking
    pub fn record_access(&self, key: &str, size: u64) {
        let mut tracker = self.access_tracker.write();
        tracker.record_access(key, size);
        tracker.cleanup_old(self.config.access_window);
    }

    /// Learn access pattern
    pub fn learn_pattern(&self, keys: &[String]) {
        if keys.len() < 2 {
            return;
        }

        let mut patterns = self.patterns.write();
        patterns.learn_pattern(keys);
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
        let warmer = CandidateWarmer::new(self.cache.clone(), self.config.clone());

        // Clone the data we need to avoid holding locks across await
        let tracker_data = {
            let tracker = self.access_tracker.read();
            tracker.clone()
        };

        let patterns_data = {
            let patterns = self.patterns.read();
            patterns.clone()
        };

        warmer.warm_cache(&tracker_data, &patterns_data).await
    }

    /// Shutdown the warmer
    pub fn shutdown(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Release);
    }
}
