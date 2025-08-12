//! Unified cache manager with security and remote cache support

mod builder;
mod keygen;
mod migration;
mod operations;
mod statistics;

pub use builder::CacheManagerBuilder;
pub use keygen::hash_task_config;
pub use migration::CACHE_VERSION;
pub use statistics::CacheStatistics;

use crate::concurrent::action::ActionCache;
use crate::config::CacheConfig;
use crate::content_addressed_store::ContentAddressedStore;
use crate::engine::CacheEngine;
use crate::keys::{CacheKeyFilterConfig, CacheKeyGenerator};
use crate::types::CachedTaskResult;
use cuenv_config::TaskConfig;
use cuenv_core::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Unified cache manager that provides access to cache components
pub struct CacheManager {
    config: CacheConfig,
    /// Store reference for future extensibility and ownership
    _content_store: Arc<ContentAddressedStore>,
    /// Store reference for future extensibility and ownership
    _action_cache: Arc<ActionCache>,
    /// Underlying cache engine
    _engine: Arc<CacheEngine>,
    /// Cache operations handler
    operations: operations::CacheOperations,
    /// Key generation manager
    key_gen_manager: keygen::KeyGenManager,
    /// Cache version for migration support
    _version: u32,
}

impl CacheManager {
    /// Create a new cache manager with given configuration (async version)
    pub async fn new(config: CacheConfig) -> Result<Self> {
        Self::new_internal(config).await
    }

    /// Create a new cache manager (sync version for main application)
    pub fn new_sync() -> Result<Self> {
        CacheManagerBuilder::new().build_sync()
    }

    /// Internal constructor shared by both sync and async versions
    pub(crate) async fn new_internal(config: CacheConfig) -> Result<Self> {
        let components = builder::initialize_components(&config).await?;

        let operations = operations::CacheOperations::new(
            Arc::clone(&components.content_store),
            Arc::clone(&components.action_cache),
            Arc::clone(&components.signer),
        );

        Ok(Self {
            config,
            _content_store: components.content_store,
            _action_cache: components.action_cache,
            _engine: components.engine,
            operations,
            key_gen_manager: components.key_gen_manager,
            _version: CACHE_VERSION,
        })
    }

    /// Clear all cache entries
    pub fn clear_cache(&self) -> Result<()> {
        self.operations.clear_cache()
    }

    /// Get the content-addressed store
    pub fn content_store(&self) -> Arc<ContentAddressedStore> {
        self.operations.content_store()
    }

    /// Get the action cache
    pub fn action_cache(&self) -> Arc<ActionCache> {
        self.operations.action_cache()
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Get cache statistics
    pub fn get_statistics(&self) -> CacheStatistics {
        self.operations.get_statistics()
    }

    /// Get cached result for a task
    pub fn get_cached_result(&self, cache_key: &str) -> Option<CachedTaskResult> {
        self.operations.get_cached_result(cache_key)
    }

    /// Store a cached result
    pub fn store_result(&self, cache_key: String, result: CachedTaskResult) -> Result<()> {
        self.operations.store_result(cache_key, result)
    }

    /// Generate cache key for a task
    pub fn generate_cache_key(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        env_vars: &HashMap<String, String>,
        working_dir: &Path,
    ) -> Result<String> {
        self.key_gen_manager
            .generate_cache_key(task_name, task_config, env_vars, working_dir)
    }

    /// Cleanup stale cache entries
    pub fn cleanup_stale_entries(&self) -> Result<()> {
        self.operations.cleanup_stale_entries()
    }

    /// Get the cache key generator for advanced configuration
    pub fn key_generator(&self) -> Arc<CacheKeyGenerator> {
        self.key_gen_manager.key_generator()
    }

    /// Apply task-specific cache environment configurations
    pub fn apply_task_configs(&mut self, tasks: &HashMap<String, TaskConfig>) -> Result<()> {
        // Extract task-specific filter configs
        let mut task_filters = HashMap::new();
        for (task_name, task_config) in tasks {
            if let Some(cache_env) = &task_config.cache_env {
                let filter_config: CacheKeyFilterConfig = cache_env.clone().into();
                task_filters.insert(task_name.clone(), filter_config.clone());
                self.config
                    .task_env_filters
                    .insert(task_name.clone(), filter_config);
            }
        }

        // Apply to key generator
        self.key_gen_manager
            .apply_task_configs(tasks, self.config.env_filter.clone())?;
        Ok(())
    }
}
