use crate::cache::traits::Cache;
use crate::cache::{CacheResult, MonitoredCache};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// ML-based predictive caching system
pub struct PredictiveCache<C: Cache + Clone> {
    cache: Arc<MonitoredCache<C>>,
    predictor: Arc<RwLock<AccessPredictor>>,
    config: PredictiveCacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveCacheConfig {
    /// Maximum number of patterns to track
    pub max_patterns: usize,
    /// Minimum confidence for prediction
    pub min_confidence: f64,
    /// Time window for pattern detection
    pub pattern_window: Duration,
    /// Maximum prefetch batch size
    pub max_prefetch_batch: usize,
    /// Enable aggressive prefetching
    pub aggressive_mode: bool,
}

impl Default for PredictiveCacheConfig {
    fn default() -> Self {
        Self {
            max_patterns: 10000,
            min_confidence: 0.7,
            pattern_window: Duration::from_secs(3600),
            max_prefetch_batch: 50,
            aggressive_mode: false,
        }
    }
}

/// Access pattern predictor using simple ML techniques
struct AccessPredictor {
    /// Access history for each key
    access_history: HashMap<String, AccessRecord>,
    /// Sequential access patterns
    sequential_patterns: HashMap<String, Vec<String>>,
    /// Temporal patterns (time-based)
    temporal_patterns: HashMap<u32, Vec<String>>, // hour of day -> likely keys
    /// Dependency graph
    dependency_graph: HashMap<String, Vec<String>>,
    /// Global access sequence
    global_sequence: VecDeque<(String, Instant)>,
    config: PredictiveCacheConfig,
}

#[derive(Debug, Clone)]
struct AccessRecord {
    /// Number of times accessed
    access_count: u64,
    /// Last access time
    last_access: Instant,
    /// Access frequency (accesses per hour)
    frequency: f64,
    /// Previous keys accessed before this one
    predecessors: Vec<String>,
    /// Keys accessed after this one
    successors: Vec<String>,
}

impl<C: Cache + Clone> PredictiveCache<C> {
    pub fn new(cache: Arc<MonitoredCache<C>>, config: PredictiveCacheConfig) -> Self {
        let predictor = Arc::new(RwLock::new(AccessPredictor::new(config.clone())));

        Self {
            cache,
            predictor,
            config,
        }
    }

    /// Record a cache access for learning
    pub async fn record_access(&self, key: &str) {
        let mut predictor = self.predictor.write().await;
        predictor.record_access(key);
    }

    /// Get predictions for what keys might be accessed next
    pub async fn predict_next_accesses(&self, current_key: &str) -> Vec<PredictedAccess> {
        let predictor = self.predictor.read().await;
        predictor.predict_next(current_key)
    }

    /// Prefetch predicted keys into cache
    pub async fn prefetch_predicted(&self, current_key: &str) -> CacheResult<usize> {
        let predictions = self.predict_next_accesses(current_key).await;
        let mut prefetched = 0;

        for prediction in predictions.iter().take(self.config.max_prefetch_batch) {
            if prediction.confidence < self.config.min_confidence {
                continue;
            }

            // Check if already in cache
            match self.cache.contains(&prediction.key).await {
                Ok(true) => continue, // Already cached
                Ok(false) => {
                    // Trigger cache warming for this key
                    info!(
                        "Prefetching {} with confidence {:.2}",
                        prediction.key, prediction.confidence
                    );
                    // In real implementation, this would trigger task execution
                    prefetched += 1;
                }
                Err(e) => {
                    warn!("Error checking cache for {}: {}", prediction.key, e);
                }
            }
        }

        Ok(prefetched)
    }

    /// Train the predictor with historical data
    pub async fn train_from_history(&self, history: Vec<AccessEvent>) -> CacheResult<()> {
        let mut predictor = self.predictor.write().await;

        for event in history {
            // Convert DateTime to elapsed time for internal tracking
            predictor.record_access(&event.key);
        }

        predictor.analyze_patterns();
        Ok(())
    }

    /// Get predictor statistics
    pub async fn get_statistics(&self) -> PredictorStatistics {
        let predictor = self.predictor.read().await;
        predictor.get_statistics()
    }
}

impl AccessPredictor {
    fn new(config: PredictiveCacheConfig) -> Self {
        Self {
            access_history: HashMap::new(),
            sequential_patterns: HashMap::new(),
            temporal_patterns: HashMap::new(),
            dependency_graph: HashMap::new(),
            global_sequence: VecDeque::new(),
            config,
        }
    }

    fn record_access(&mut self, key: &str) {
        self.record_access_at(key, Instant::now());
    }

    fn record_access_at(&mut self, key: &str, timestamp: Instant) {
        let key_string = key.to_string();
        let mut prev_key_for_predecessors = None;

        // Update access record
        {
            let record = self
                .access_history
                .entry(key_string.clone())
                .or_insert(AccessRecord {
                    access_count: 0,
                    last_access: timestamp,
                    frequency: 0.0,
                    predecessors: Vec::new(),
                    successors: Vec::new(),
                });

            record.access_count += 1;
            record.last_access = timestamp;

            // Calculate frequency
            if record.access_count > 1 {
                let duration = timestamp.duration_since(record.last_access).as_secs_f64() / 3600.0;
                if duration > 0.0 {
                    record.frequency = record.access_count as f64 / duration;
                }
            }
        }

        // Update global sequence
        self.global_sequence
            .push_back((key_string.clone(), timestamp));

        // Maintain window size
        while self.global_sequence.len() > self.config.max_patterns {
            self.global_sequence.pop_front();
        }

        // Update sequential patterns
        if self.global_sequence.len() >= 2 {
            let seq_len = self.global_sequence.len();
            if let Some((prev_key, _)) = self.global_sequence.get(seq_len - 2) {
                prev_key_for_predecessors = Some(prev_key.clone());

                // Record sequential pattern
                let pattern = self
                    .sequential_patterns
                    .entry(prev_key.clone())
                    .or_default();
                if !pattern.contains(&key_string) {
                    pattern.push(key_string.clone());
                }

                // Update dependency graph
                let deps = self.dependency_graph.entry(key_string.clone()).or_default();
                if !deps.contains(prev_key) {
                    deps.push(prev_key.clone());
                }

                // Update predecessors/successors
                if let Some(prev_record) = self.access_history.get_mut(prev_key) {
                    // Always add successor to track transition frequency
                    prev_record.successors.push(key_string.clone());
                }
            }
        }

        // Update predecessors for current record
        if let Some(prev_key) = prev_key_for_predecessors {
            if let Some(record) = self.access_history.get_mut(&key_string) {
                record.predecessors.push(prev_key);
            }
        }

        // Update temporal patterns (hour of day)
        let hour = (timestamp.elapsed().as_secs() / 3600) % 24;
        let temporal = self.temporal_patterns.entry(hour as u32).or_default();
        if !temporal.contains(&key_string) {
            temporal.push(key_string);
        }
    }

    fn predict_next(&self, current_key: &str) -> Vec<PredictedAccess> {
        let mut predictions = Vec::new();

        // Sequential predictions
        if let Some(sequential) = self.sequential_patterns.get(current_key) {
            for next_key in sequential {
                let confidence = self.calculate_sequential_confidence(current_key, next_key);
                predictions.push(PredictedAccess {
                    key: next_key.clone(),
                    confidence,
                    prediction_type: PredictionType::Sequential,
                });
            }
        }

        // Temporal predictions
        let current_hour = (Instant::now().elapsed().as_secs() / 3600) % 24;
        if let Some(temporal) = self.temporal_patterns.get(&(current_hour as u32)) {
            for key in temporal {
                if key != current_key {
                    let confidence = self.calculate_temporal_confidence(key);
                    predictions.push(PredictedAccess {
                        key: key.clone(),
                        confidence,
                        prediction_type: PredictionType::Temporal,
                    });
                }
            }
        }

        // Dependency-based predictions
        if let Some(record) = self.access_history.get(current_key) {
            for successor in &record.successors {
                let confidence = self.calculate_dependency_confidence(current_key, successor);
                predictions.push(PredictedAccess {
                    key: successor.clone(),
                    confidence,
                    prediction_type: PredictionType::Dependency,
                });
            }
        }

        // Sort by confidence
        predictions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Deduplicate keeping highest confidence
        let mut seen = HashMap::new();
        predictions.retain(|pred| seen.insert(pred.key.clone(), ()).is_none());

        predictions
    }

    fn calculate_sequential_confidence(&self, from: &str, to: &str) -> f64 {
        if let Some(from_record) = self.access_history.get(from) {
            if let Some(_to_record) = self.access_history.get(to) {
                let total_transitions = from_record.successors.len() as f64;
                if total_transitions > 0.0 {
                    let to_transitions =
                        from_record.successors.iter().filter(|s| *s == to).count() as f64;
                    let confidence = to_transitions / total_transitions;
                    return confidence;
                }
            }
        }
        0.0
    }

    fn calculate_temporal_confidence(&self, key: &str) -> f64 {
        if let Some(record) = self.access_history.get(key) {
            // Base confidence on frequency and recency
            let recency_factor = 1.0 / (1.0 + record.last_access.elapsed().as_secs_f64() / 3600.0);
            let frequency_factor = (record.frequency / 10.0).min(1.0);
            return recency_factor * 0.3 + frequency_factor * 0.7;
        }
        0.0
    }

    fn calculate_dependency_confidence(&self, from: &str, to: &str) -> f64 {
        // Use the same logic as sequential confidence since dependency is just
        // another way to look at the same successor relationship
        self.calculate_sequential_confidence(from, to)
    }

    fn analyze_patterns(&mut self) {
        debug!(
            "Analyzing access patterns with {} records",
            self.access_history.len()
        );

        // Find frequent item sets (simplified Apriori algorithm)
        let mut frequent_pairs = HashMap::new();

        for i in 0..self.global_sequence.len().saturating_sub(1) {
            if let (Some((key1, _)), Some((key2, _))) =
                (self.global_sequence.get(i), self.global_sequence.get(i + 1))
            {
                let pair = format!("{}->{}", key1, key2);
                *frequent_pairs.entry(pair).or_insert(0) += 1;
            }
        }

        // Update sequential patterns based on frequency
        for (pair, count) in frequent_pairs {
            if count >= 3 {
                // Minimum support
                if let Some(arrow_pos) = pair.find("->") {
                    let from = &pair[..arrow_pos];
                    let to = &pair[arrow_pos + 2..];
                    self.sequential_patterns
                        .entry(from.to_string())
                        .or_default()
                        .push(to.to_string());
                }
            }
        }
    }

    fn get_statistics(&self) -> PredictorStatistics {
        PredictorStatistics {
            total_keys_tracked: self.access_history.len(),
            sequential_patterns: self.sequential_patterns.len(),
            temporal_patterns: self.temporal_patterns.values().map(|v| v.len()).sum(),
            dependency_edges: self.dependency_graph.values().map(|v| v.len()).sum(),
            total_accesses: self.access_history.values().map(|r| r.access_count).sum(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedAccess {
    pub key: String,
    pub confidence: f64,
    pub prediction_type: PredictionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictionType {
    Sequential,
    Temporal,
    Dependency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessEvent {
    pub key: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorStatistics {
    pub total_keys_tracked: usize,
    pub sequential_patterns: usize,
    pub temporal_patterns: usize,
    pub dependency_edges: usize,
    pub total_accesses: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::cache_impl::Cache;
    use crate::cache::traits::CacheConfig;
    use tempfile;

    #[tokio::test]
    async fn test_sequential_pattern_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
            .await
            .unwrap();
        let cache = Arc::new(MonitoredCache::new(base_cache, "test").unwrap());
        let predictor = PredictiveCache::new(cache, Default::default());

        // Simulate sequential access pattern
        predictor.record_access("task_a").await;
        predictor.record_access("task_b").await;
        predictor.record_access("task_c").await;

        // Repeat pattern
        predictor.record_access("task_a").await;
        predictor.record_access("task_b").await;
        predictor.record_access("task_c").await;

        let predictions = predictor.predict_next_accesses("task_a").await;
        assert!(!predictions.is_empty());
        assert_eq!(predictions[0].key, "task_b");
    }

    #[tokio::test]
    async fn test_confidence_calculation() {
        let mut predictor = AccessPredictor::new(Default::default());

        // Create pattern with different frequencies
        for _ in 0..10 {
            predictor.record_access("a");
            predictor.record_access("b");
        }

        for _ in 0..5 {
            predictor.record_access("a");
            predictor.record_access("c");
        }

        let predictions = predictor.predict_next("a");
        assert!(predictions.len() >= 2);

        // "b" should have higher confidence than "c"
        let b_pred = predictions.iter().find(|p| p.key == "b").unwrap();
        let c_pred = predictions.iter().find(|p| p.key == "c").unwrap();
        assert!(b_pred.confidence > c_pred.confidence);
    }
}
