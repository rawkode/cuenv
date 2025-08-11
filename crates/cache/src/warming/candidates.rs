//! Candidate selection and warming logic

use super::patterns::WarmingPatterns;
use super::tracker::AccessTracker;
use super::types::WarmingConfig;
use crate::errors::{CacheError, RecoveryHint, Result};
use crate::traits::Cache;
use std::collections::HashSet;

/// Candidate selection and warming operations
pub(crate) struct CandidateWarmer<C: Cache> {
    cache: C,
    config: WarmingConfig,
}

impl<C: Cache + Clone> CandidateWarmer<C> {
    pub fn new(cache: C, config: WarmingConfig) -> Self {
        Self { cache, config }
    }

    /// Perform cache warming
    pub async fn warm_cache(
        &self,
        tracker: &AccessTracker,
        patterns: &WarmingPatterns,
    ) -> Result<usize> {
        // Get candidates for warming
        let candidates = self.get_warming_candidates(tracker, patterns);

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
    fn get_warming_candidates(
        &self,
        tracker: &AccessTracker,
        patterns: &WarmingPatterns,
    ) -> Vec<(String, u64)> {
        let mut candidates = if self.config.max_warming_size > 0 {
            // Use size-aware selection if max_warming_size is set
            tracker.get_candidates_by_size(self.config.max_warming_size)
        } else {
            // Use standard access-count based selection
            let mut c = tracker.get_candidates(self.config.min_access_count);
            // Sort by access count (descending)
            c.sort_by(|a, b| b.1.cmp(&a.1));
            c
        };

        // Add predictive candidates if enabled
        if self.config.predictive_warming {
            self.add_predictive_candidates(&mut candidates, patterns);
        }

        candidates
    }

    /// Add predictive warming candidates
    fn add_predictive_candidates(
        &self,
        candidates: &mut Vec<(String, u64)>,
        patterns: &WarmingPatterns,
    ) {
        let selected_keys: HashSet<_> = candidates.iter().map(|(k, _)| k.clone()).collect();
        let predictive = patterns.get_predictive_candidates(&selected_keys);

        for key in predictive {
            if !candidates.iter().any(|(k, _)| k == &key) {
                candidates.push((key, self.config.min_access_count));
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
                message: format!("Failed to warm cache entry: {e}"),
                recovery_hint: RecoveryHint::Ignore,
            }),
        }
    }
}
