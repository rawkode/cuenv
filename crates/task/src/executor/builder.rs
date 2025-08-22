use super::{cache, TaskExecutor};
use crate::{MonorepoTaskRegistry, TaskBuilder};
use cuenv_cache::config::CacheConfiguration;
use cuenv_cache::CacheManager;
use cuenv_core::Result;
use cuenv_env::manager::EnvManager;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

impl TaskExecutor {
    /// Create a new task executor
    pub async fn new(env_manager: EnvManager, working_dir: PathBuf) -> Result<Self> {
        // TODO: Add CacheConfigLoader when moved to workspace
        let cache_config = CacheConfiguration::default();
        let cache_config_struct = cache::create_cache_config_struct(&cache_config)?;
        let mut cache_manager = CacheManager::new(cache_config_struct).await?;

        // Apply task-specific cache environment configurations
        let tasks = env_manager.get_tasks();
        cache_manager.apply_task_configs(tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with current working directory and environment
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config,
            task_builder,
            monorepo_registry: None,
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Create a new task executor with monorepo registry for cross-package execution
    pub async fn new_with_registry(registry: MonorepoTaskRegistry) -> Result<Self> {
        // Create a minimal env manager for the registry-based executor
        let env_manager = EnvManager::new();
        let working_dir = std::env::current_dir()?;

        // TODO: Add CacheConfigLoader when moved to workspace
        let cache_config = CacheConfiguration::default();
        let cache_config_struct = cache::create_cache_config_struct(&cache_config)?;
        let cache_manager = CacheManager::new(cache_config_struct).await?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with current working directory
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config,
            task_builder,
            monorepo_registry: Some(Arc::new(registry)),
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Create a new task executor with custom cache config (for testing)
    #[cfg(test)]
    pub async fn new_with_config(
        env_manager: EnvManager,
        working_dir: PathBuf,
        cache_config: cuenv_cache::CacheConfig,
    ) -> Result<Self> {
        let cache_configuration = CacheConfiguration::default();
        let mut cache_manager = CacheManager::new(cache_config).await?;

        // Apply task-specific cache environment configurations
        let tasks = env_manager.get_tasks();
        cache_manager.apply_task_configs(tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with current working directory
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config: cache_configuration,
            task_builder,
            monorepo_registry: None,
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}
