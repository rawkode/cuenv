//! Cache manager builder and initialization

use super::keygen::KeyGenManager;
use super::migration::CacheMigrator;
use super::operations::CacheOperations;
use super::statistics::StatsContainer;
use crate::concurrent::action::ActionCache;
use crate::config::CacheConfig;
use crate::content_addressed_store::ContentAddressedStore;
use crate::engine::CacheEngine;
use crate::keys::{CacheKeyFilterConfig, CacheKeyGenerator};
use crate::security::signing::CacheSigner;
use cuenv_core::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Builder for CacheManager
pub struct CacheManagerBuilder {
    config: Option<CacheConfig>,
    base_dir: Option<PathBuf>,
    max_size: Option<u64>,
    inline_threshold: Option<usize>,
    env_filter: Option<CacheKeyFilterConfig>,
}

impl CacheManagerBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            base_dir: None,
            max_size: None,
            inline_threshold: None,
            env_filter: None,
        }
    }

    pub fn with_config(mut self, config: CacheConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.base_dir = Some(dir);
        self
    }

    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_size = Some(size);
        self
    }

    pub fn with_inline_threshold(mut self, threshold: usize) -> Self {
        self.inline_threshold = Some(threshold);
        self
    }

    pub fn with_env_filter(mut self, filter: CacheKeyFilterConfig) -> Self {
        self.env_filter = Some(filter);
        self
    }

    /// Build the cache manager asynchronously
    pub async fn build_async(self) -> Result<super::CacheManager> {
        let config = self.build_config()?;
        super::CacheManager::new_internal(config).await
    }

    /// Build the cache manager synchronously
    pub fn build_sync(self) -> Result<super::CacheManager> {
        let config = self.build_config()?;

        // Check if we're already in an async context
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(Error::configuration(
                "Cannot use sync constructor from async context. Use build_async() instead."
                    .to_string(),
            ));
        }

        // Create a new runtime for this operation
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::configuration(format!("Failed to create tokio runtime: {e}")))?;

        rt.block_on(super::CacheManager::new_internal(config))
    }

    fn build_config(self) -> Result<CacheConfig> {
        if let Some(config) = self.config {
            Ok(config)
        } else {
            let base_dir = self.base_dir.unwrap_or_else(|| {
                let engine = CacheEngine::new().unwrap();
                engine.cache_dir.clone()
            });

            Ok(CacheConfig {
                base_dir,
                max_size: self.max_size.unwrap_or(1024 * 1024 * 1024), // 1GB default
                mode: super::super::CacheMode::ReadWrite,
                inline_threshold: self.inline_threshold.unwrap_or(4096), // 4KB default
                env_filter: self.env_filter.unwrap_or_default(),
                task_env_filters: HashMap::new(),
            })
        }
    }
}

impl Default for CacheManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize cache components
pub async fn initialize_components(config: &CacheConfig) -> Result<CacheComponents> {
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
        &config.base_dir,
    )?);

    // Initialize cache engine for legacy compatibility
    let engine = Arc::new(CacheEngine::new().map_err(|e| Error::Configuration {
        message: format!("Failed to initialize cache engine: {e}"),
    })?);

    // Initialize signer
    let signer =
        Arc::new(
            CacheSigner::new(&config.base_dir).map_err(|e| Error::Configuration {
                message: format!("Failed to initialize cache signer: {e}"),
            })?,
        );

    // Initialize cache key generator with configuration
    let mut key_gen_manager = KeyGenManager::new(config.env_filter.clone())?;

    // Add task-specific configurations
    for (task_name, task_config) in &config.task_env_filters {
        let mut key_gen = CacheKeyGenerator::with_config(config.env_filter.clone())?;
        key_gen.add_task_config(task_name, task_config.clone())?;
    }

    // Initialize migrator
    let migrator = CacheMigrator::new();
    migrator.check_and_migrate(&config.base_dir)?;

    Ok(CacheComponents {
        content_store,
        action_cache,
        engine,
        signer,
        key_gen_manager,
        operations: None, // Will be created later
        stats: StatsContainer::new(),
    })
}

/// Container for cache components
pub struct CacheComponents {
    pub content_store: Arc<ContentAddressedStore>,
    pub action_cache: Arc<ActionCache>,
    pub engine: Arc<CacheEngine>,
    pub signer: Arc<CacheSigner>,
    pub key_gen_manager: KeyGenManager,
    pub operations: Option<CacheOperations>,
    pub stats: StatsContainer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_builder() -> Result<()> {
        let temp_dir = TempDir::new()?;

        let builder = CacheManagerBuilder::new()
            .with_base_dir(temp_dir.path().to_path_buf())
            .with_max_size(1024 * 1024)
            .with_inline_threshold(2048);

        let config = builder.build_config()?;
        assert_eq!(config.max_size, 1024 * 1024);
        assert_eq!(config.inline_threshold, 2048);

        Ok(())
    }
}
