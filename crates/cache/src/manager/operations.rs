//! Cache operations - get, store, and cleanup

use super::statistics::StatsContainer;
use crate::concurrent::action::{ActionCache, ActionResult};
use crate::content_addressed_store::ContentAddressedStore;
use crate::security::signing::CacheSigner;
use crate::types::CachedTaskResult;
use cuenv_core::{Error, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Handles cache operations (get, store, cleanup)
pub struct CacheOperations {
    content_store: Arc<ContentAddressedStore>,
    action_cache: Arc<ActionCache>,
    signer: Arc<CacheSigner>,
    memory_cache: Arc<Mutex<HashMap<String, CachedTaskResult>>>,
    stats: StatsContainer,
}

impl CacheOperations {
    pub fn new(
        content_store: Arc<ContentAddressedStore>,
        action_cache: Arc<ActionCache>,
        signer: Arc<CacheSigner>,
    ) -> Self {
        Self {
            content_store,
            action_cache,
            signer,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
            stats: StatsContainer::new(),
        }
    }

    /// Get cached result for a task
    pub fn get_cached_result(&self, cache_key: &str) -> Option<CachedTaskResult> {
        // First try to get from ActionCache
        if let Some(action_result) = self.action_cache.get_cached_action_result(cache_key) {
            // Convert ActionResult back to CachedTaskResult for backward compatibility
            let cached_result = self.convert_action_result(cache_key, action_result);

            // Only return successful results (exit_code == 0)
            if cached_result.exit_code == 0 {
                self.stats.record_hit();
                return Some(cached_result);
            }
        }

        // Fallback to in-memory cache for backward compatibility
        if let Ok(cache) = self.memory_cache.lock() {
            if let Some(result) = cache.get(cache_key) {
                // Only return successful results (exit_code == 0)
                if result.exit_code == 0 {
                    self.stats.record_hit();
                    return Some(result.clone());
                }
            }
        }

        self.stats.record_miss();
        None
    }

    /// Store a cached result
    pub fn store_result(&self, cache_key: String, result: CachedTaskResult) -> Result<()> {
        // Sign the result for integrity
        let _signed_result = self
            .signer
            .sign(&result)
            .map_err(|e| Error::Configuration {
                message: format!("Failed to sign cache result: {e}"),
            })?;

        // Only cache successful results (exit_code == 0)
        if result.exit_code == 0 {
            // Store in ActionCache (this will be handled by ActionCache::execute_action)
            // For backward compatibility, also store in memory cache
            if let Ok(mut cache) = self.memory_cache.lock() {
                cache.insert(cache_key.clone(), result.clone());
            }
        }

        self.stats.record_write();
        Ok(())
    }

    /// Cleanup stale cache entries
    pub fn cleanup_stale_entries(&self) -> Result<()> {
        // Run garbage collection on content store
        let (removed_count, removed_bytes) = self.content_store.garbage_collect()?;

        log::info!("Cache cleanup: removed {removed_count} entries, freed {removed_bytes} bytes");

        self.stats.record_cleanup();
        Ok(())
    }

    /// Clear all cache entries
    pub fn clear_cache(&self) -> Result<()> {
        // Clear action cache
        self.action_cache.clear();
        
        // Clear memory cache
        if let Ok(mut cache) = self.memory_cache.lock() {
            cache.clear();
        }

        log::info!("Cache cleared");
        Ok(())
    }

    /// Get the content-addressed store
    pub fn content_store(&self) -> Arc<ContentAddressedStore> {
        Arc::clone(&self.content_store)
    }

    /// Get the action cache
    pub fn action_cache(&self) -> Arc<ActionCache> {
        Arc::clone(&self.action_cache)
    }

    /// Get statistics
    pub fn get_statistics(&self) -> super::statistics::CacheStatistics {
        self.stats.get_snapshot()
    }

    /// Convert ActionResult to CachedTaskResult
    fn convert_action_result(&self, cache_key: &str, action_result: ActionResult) -> CachedTaskResult {
        CachedTaskResult {
            cache_key: cache_key.to_string(),
            executed_at: action_result.executed_at,
            exit_code: action_result.exit_code,
            stdout: action_result.stdout_hash.map(|s| s.as_bytes().to_vec()),
            stderr: action_result.stderr_hash.map(|s| s.as_bytes().to_vec()),
            output_files: action_result.output_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cas_dir = temp_dir.path().join("cas");
        std::fs::create_dir_all(&cas_dir)?;

        let content_store = Arc::new(ContentAddressedStore::new(cas_dir, 4096)?);
        let action_cache = Arc::new(ActionCache::new(
            Arc::clone(&content_store),
            1024 * 1024,
            temp_dir.path(),
        )?);
        let signer = Arc::new(CacheSigner::new(temp_dir.path())?);

        let operations = CacheOperations::new(content_store, action_cache, signer);

        // Test storing and retrieving
        let result = CachedTaskResult {
            cache_key: "test_key".to_string(),
            executed_at: SystemTime::now(),
            exit_code: 0,
            stdout: Some(b"output".to_vec()),
            stderr: None,
            output_files: HashMap::new(),
        };

        operations.store_result("test_key".to_string(), result.clone())?;
        
        // Should be able to retrieve from memory cache
        let retrieved = operations.get_cached_result("test_key");
        assert!(retrieved.is_some());

        Ok(())
    }
}