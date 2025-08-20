//! DAG cache for performance optimization
//!
//! This module provides caching for unified DAG builds to improve performance
//! when executing the same tasks repeatedly.

use super::unified_dag::UnifiedTaskDAG;
use cuenv_config::{TaskConfig, TaskNode};
use cuenv_core::Result;
use indexmap::IndexMap;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Cache entry for a built DAG
#[derive(Clone)]
struct DAGCacheEntry {
    /// The cached unified DAG
    dag: UnifiedTaskDAG,
    /// When this entry was created
    created_at: Instant,
    /// Hash of the task configurations used to build this DAG
    config_hash: u64,
}

/// DAG cache configuration
#[derive(Clone)]
pub struct DAGCacheConfig {
    /// Maximum number of entries to cache
    pub max_entries: usize,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Enable/disable the cache
    pub enabled: bool,
}

impl Default for DAGCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 100,
            ttl: Duration::from_secs(300), // 5 minutes
            enabled: true,
        }
    }
}

/// Cache for unified DAG builds
pub struct DAGCache {
    /// Internal cache storage
    cache: Arc<Mutex<HashMap<String, DAGCacheEntry>>>,
    /// Cache configuration
    config: DAGCacheConfig,
}

impl DAGCache {
    /// Create a new DAG cache with default configuration
    pub fn new() -> Self {
        Self::with_config(DAGCacheConfig::default())
    }

    /// Create a new DAG cache with custom configuration
    pub fn with_config(config: DAGCacheConfig) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Generate a cache key for the given task names and configuration hash
    fn cache_key(&self, task_names: &[String], config_hash: u64) -> String {
        let mut task_names_sorted = task_names.to_vec();
        task_names_sorted.sort();
        format!("{}:{:x}", task_names_sorted.join(","), config_hash)
    }

    /// Calculate hash of task configurations for cache invalidation
    pub fn calculate_config_hash(
        &self,
        task_configs: &HashMap<String, TaskConfig>,
        task_nodes: &IndexMap<String, TaskNode>,
    ) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash task configs in sorted order for consistency
        let mut sorted_configs: Vec<_> = task_configs.iter().collect();
        sorted_configs.sort_by_key(|(name, _)| name.as_str());
        for (name, config) in sorted_configs {
            name.hash(&mut hasher);
            // Hash individual fields since TaskConfig may not implement Hash
            config.description.hash(&mut hasher);
            config.command.hash(&mut hasher);
            config.script.hash(&mut hasher);
            config.dependencies.hash(&mut hasher);
            config.working_dir.hash(&mut hasher);
            config.shell.hash(&mut hasher);
            config.inputs.hash(&mut hasher);
            config.outputs.hash(&mut hasher);
            // Note: skipping env since it's HashMap which may not have stable ordering
        }

        // Hash task nodes in insertion order (IndexMap preserves this)
        for (name, node) in task_nodes {
            name.hash(&mut hasher);
            // Hash based on discriminant to distinguish node types
            std::mem::discriminant(node).hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Get a cached DAG if available and valid
    pub fn get(&self, task_names: &[String], config_hash: u64) -> Option<UnifiedTaskDAG> {
        if !self.config.enabled {
            return None;
        }

        let cache_key = self.cache_key(task_names, config_hash);
        let cache = self.cache.lock().ok()?;

        if let Some(entry) = cache.get(&cache_key) {
            // Check if entry is still valid
            if entry.config_hash == config_hash && entry.created_at.elapsed() < self.config.ttl {
                return Some(entry.dag.clone());
            }
        }

        None
    }

    /// Store a DAG in the cache
    pub fn put(&self, task_names: &[String], config_hash: u64, dag: UnifiedTaskDAG) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let cache_key = self.cache_key(task_names, config_hash);
        let entry = DAGCacheEntry {
            dag,
            created_at: Instant::now(),
            config_hash,
        };

        let mut cache = self
            .cache
            .lock()
            .map_err(|_| cuenv_core::Error::configuration("Failed to acquire cache lock"))?;

        // Evict old entries if we're at capacity
        if cache.len() >= self.config.max_entries {
            self.evict_oldest_entries(&mut cache);
        }

        cache.insert(cache_key, entry);
        Ok(())
    }

    /// Evict oldest entries when at capacity
    fn evict_oldest_entries(&self, cache: &mut HashMap<String, DAGCacheEntry>) {
        if cache.is_empty() {
            return;
        }

        // Find the oldest 25% of entries to remove
        let evict_count = (self.config.max_entries / 4).max(1);
        let mut entries: Vec<(String, Instant)> = cache
            .iter()
            .map(|(key, entry)| (key.clone(), entry.created_at))
            .collect();

        // Sort by creation time (oldest first)
        entries.sort_by_key(|(_, created_at)| *created_at);

        // Remove the oldest entries
        for (key, _) in entries.into_iter().take(evict_count) {
            cache.remove(&key);
        }
    }

    /// Clear expired entries from the cache
    pub fn cleanup_expired(&self) -> Result<usize> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| cuenv_core::Error::configuration("Failed to acquire cache lock"))?;

        let now = Instant::now();
        let initial_size = cache.len();

        cache.retain(|_, entry| now.duration_since(entry.created_at) < self.config.ttl);

        Ok(initial_size - cache.len())
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<DAGCacheStats> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| cuenv_core::Error::configuration("Failed to acquire cache lock"))?;

        let now = Instant::now();
        let total_entries = cache.len();
        let expired_entries = cache
            .values()
            .filter(|entry| now.duration_since(entry.created_at) >= self.config.ttl)
            .count();

        Ok(DAGCacheStats {
            total_entries,
            valid_entries: total_entries - expired_entries,
            expired_entries,
            max_entries: self.config.max_entries,
        })
    }

    /// Clear all entries from the cache
    pub fn clear(&self) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| cuenv_core::Error::configuration("Failed to acquire cache lock"))?;

        cache.clear();
        Ok(())
    }
}

/// Statistics about the DAG cache
#[derive(Debug, Clone)]
pub struct DAGCacheStats {
    /// Total number of entries in cache
    pub total_entries: usize,
    /// Number of valid (non-expired) entries
    pub valid_entries: usize,
    /// Number of expired entries
    pub expired_entries: usize,
    /// Maximum allowed entries
    pub max_entries: usize,
}

impl Default for DAGCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::TaskConfig;
    use std::collections::HashMap;
    use std::thread;

    fn create_test_config(command: &str, deps: Option<Vec<String>>) -> TaskConfig {
        TaskConfig {
            command: Some(command.to_string()),
            dependencies: deps,
            ..Default::default()
        }
    }

    #[test]
    fn test_dag_cache_basic_operations() {
        let cache = DAGCache::new();
        let task_names = vec!["test".to_string()];
        let config_hash = 12345u64;

        // Initially should be empty
        assert!(cache.get(&task_names, config_hash).is_none());

        // Create a mock DAG (using builder)
        let mut task_configs = HashMap::new();
        task_configs.insert("test".to_string(), create_test_config("echo test", None));

        let dag = crate::executor::unified_dag::UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .build_for_tasks(&task_names)
            .unwrap();

        // Store in cache
        cache.put(&task_names, config_hash, dag.clone()).unwrap();

        // Should now be available
        let cached_dag = cache.get(&task_names, config_hash);
        assert!(cached_dag.is_some());

        // Different config hash should not match
        assert!(cache.get(&task_names, 54321u64).is_none());
    }

    #[test]
    fn test_dag_cache_expiration() {
        let config = DAGCacheConfig {
            ttl: Duration::from_millis(50), // Very short TTL for testing
            ..Default::default()
        };
        let cache = DAGCache::with_config(config);
        let task_names = vec!["test".to_string()];
        let config_hash = 12345u64;

        let mut task_configs = HashMap::new();
        task_configs.insert("test".to_string(), create_test_config("echo test", None));

        let dag = crate::executor::unified_dag::UnifiedTaskDAG::builder()
            .with_task_configs(task_configs)
            .build_for_tasks(&task_names)
            .unwrap();

        cache.put(&task_names, config_hash, dag).unwrap();

        // Should be available immediately
        assert!(cache.get(&task_names, config_hash).is_some());

        // Wait for expiration
        thread::sleep(Duration::from_millis(100));

        // Should now be expired
        assert!(cache.get(&task_names, config_hash).is_none());
    }

    #[test]
    fn test_dag_cache_eviction() {
        let config = DAGCacheConfig {
            max_entries: 2, // Very small cache for testing
            ..Default::default()
        };
        let cache = DAGCache::with_config(config);

        // Fill cache to capacity
        for i in 0..3 {
            let task_names = vec![format!("test{}", i)];
            let mut task_configs = HashMap::new();
            task_configs.insert(
                format!("test{i}"),
                create_test_config(&format!("echo test{i}"), None),
            );

            let dag = crate::executor::unified_dag::UnifiedTaskDAG::builder()
                .with_task_configs(task_configs)
                .build_for_tasks(&task_names)
                .unwrap();
            cache.put(&task_names, i as u64, dag).unwrap();

            // Add small delay to ensure different creation times
            thread::sleep(Duration::from_millis(1));
        }

        // Cache should have evicted oldest entries
        let stats = cache.stats().unwrap();
        assert!(stats.total_entries <= 2);
    }

    #[test]
    fn test_cache_key_generation() {
        let cache = DAGCache::new();

        // Same tasks in different order should generate same key
        let key1 = cache.cache_key(&["a".to_string(), "b".to_string()], 123);
        let key2 = cache.cache_key(&["b".to_string(), "a".to_string()], 123);
        assert_eq!(key1, key2);

        // Different config hash should generate different key
        let key3 = cache.cache_key(&["a".to_string(), "b".to_string()], 456);
        assert_ne!(key1, key3);
    }
}
