//! Cache key generation utilities

use crate::keys::{CacheKeyFilterConfig, CacheKeyGenerator};
use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Cache key generation manager
pub struct KeyGenManager {
    key_generator: Arc<CacheKeyGenerator>,
}

impl KeyGenManager {
    /// Create a new key generation manager
    pub fn new(env_filter: CacheKeyFilterConfig) -> Result<Self> {
        let key_generator = CacheKeyGenerator::with_config(env_filter)?;
        Ok(Self {
            key_generator: Arc::new(key_generator),
        })
    }

    /// Create with existing generator
    #[allow(dead_code)]
    pub fn with_generator(key_generator: Arc<CacheKeyGenerator>) -> Self {
        Self { key_generator }
    }

    /// Generate cache key for a task
    pub fn generate_cache_key(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        env_vars: &HashMap<String, String>,
        working_dir: &Path,
    ) -> Result<String> {
        // Use the selective cache key generator for improved cache hit rates
        let config_hash = hash_task_config(task_config)?;
        let command = task_config.command.as_ref().or(task_config.script.as_ref());

        // For now, use empty input files since we don't have them in this context
        let input_files = HashMap::new();

        self.key_generator
            .generate_cache_key(
                task_name,
                &config_hash,
                working_dir,
                &input_files,
                env_vars,
                command.map(|s| s.as_str()),
            )
            .map_err(Into::into)
    }

    /// Legacy API for backward compatibility with tests
    pub fn generate_cache_key_legacy(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
    ) -> Result<String> {
        // Use empty env vars for deterministic testing
        let env_vars = HashMap::new();
        self.generate_cache_key(task_name, task_config, &env_vars, working_dir)
    }

    /// Apply task-specific cache environment configurations
    pub fn apply_task_configs(
        &mut self,
        tasks: &HashMap<String, TaskConfig>,
        env_filter: CacheKeyFilterConfig,
    ) -> Result<()> {
        // Create a new key generator with the current global config
        let mut new_key_generator = CacheKeyGenerator::with_config(env_filter)?;

        // Process each task to extract cache environment configurations
        for (task_name, task_config) in tasks {
            if let Some(cache_env) = &task_config.cache_env {
                // Convert CacheEnvConfig to CacheKeyFilterConfig
                let filter_config: CacheKeyFilterConfig = cache_env.clone().into();

                // Add task-specific configuration
                new_key_generator.add_task_config(task_name, filter_config)?;
            }
        }

        // Replace the key generator with the updated one
        self.key_generator = Arc::new(new_key_generator);
        Ok(())
    }

    /// Get the underlying key generator
    pub fn key_generator(&self) -> Arc<CacheKeyGenerator> {
        Arc::clone(&self.key_generator)
    }
}

/// Compute hash of task configuration for cache key generation
pub fn hash_task_config(config: &TaskConfig) -> Result<String> {
    let serialized = serde_json::to_string(config).map_err(|e| Error::Json {
        message: "Failed to serialize task config for hashing".to_string(),
        source: e,
    })?;

    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_task_config() -> Result<()> {
        let config = TaskConfig {
            command: Some("echo test".to_string()),
            ..Default::default()
        };

        let hash1 = hash_task_config(&config)?;
        let hash2 = hash_task_config(&config)?;

        // Same config should produce same hash
        assert_eq!(hash1, hash2);

        // Different config should produce different hash
        let config2 = TaskConfig {
            command: Some("echo different".to_string()),
            ..Default::default()
        };
        let hash3 = hash_task_config(&config2)?;
        assert_ne!(hash1, hash3);

        Ok(())
    }

    #[test]
    fn test_key_generation() -> Result<()> {
        let filter = CacheKeyFilterConfig::default();
        let manager = KeyGenManager::new(filter)?;

        let config = TaskConfig {
            command: Some("echo test".to_string()),
            ..Default::default()
        };

        let key = manager.generate_cache_key_legacy("test_task", &config, Path::new("/test"))?;

        assert!(!key.is_empty());
        // The key is a hash, so it won't contain the literal task name
        // Just verify it's a valid hex string
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));

        Ok(())
    }
}
