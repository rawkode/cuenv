//! Extended cache manager for remote cache integration
use super::{ActionCache, CacheConfig, ContentAddressedStore};
use anyhow::Result;
use std::sync::Arc;

/// Extended cache manager that provides access to cache components
pub struct CacheManager {
    config: CacheConfig,
    content_store: Arc<ContentAddressedStore>,
    action_cache: Arc<ActionCache>,
}

impl CacheManager {
    /// Create a new cache manager with given configuration
    pub async fn new(config: CacheConfig) -> Result<Self> {
        // Create cache directories
        std::fs::create_dir_all(&config.base_dir)?;

        let cas_dir = config.base_dir.join("cas");
        let action_dir = config.base_dir.join("actions");

        std::fs::create_dir_all(&cas_dir)?;
        std::fs::create_dir_all(&action_dir)?;

        // Initialize content-addressed store
        let content_store = Arc::new(ContentAddressedStore::new(
            cas_dir,
            config.inline_threshold,
        )?);

        // Initialize action cache with CAS and max size
        let action_cache = Arc::new(ActionCache::new(
            Arc::clone(&content_store),
            config.max_size,
        ));

        Ok(Self {
            config,
            content_store,
            action_cache,
        })
    }

    /// Get the content-addressed store
    pub fn content_store(&self) -> Arc<ContentAddressedStore> {
        Arc::clone(&self.content_store)
    }

    /// Get the action cache
    pub fn action_cache(&self) -> Arc<ActionCache> {
        Arc::clone(&self.action_cache)
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}
