//! Unified task manager that supports both consuming external tasks and providing cuenv tasks

use super::manager::TaskServerManager;
use super::provider::TaskServerProvider;
use super::types::TaskDefinition;
use cuenv_config::Config;
use cuenv_core::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Unified task manager that supports both consuming external tasks and providing cuenv tasks
pub struct UnifiedTaskManager {
    /// Manager for consuming external task servers
    pub server_manager: TaskServerManager,
    /// Provider for exposing cuenv tasks (optional)
    pub server_provider: Option<TaskServerProvider>,
    /// Configuration containing tasks
    pub config: Arc<Config>,
}

impl UnifiedTaskManager {
    /// Create a new unified task manager
    pub fn new(socket_dir: PathBuf, config: Arc<Config>) -> Self {
        Self {
            server_manager: TaskServerManager::new(socket_dir),
            server_provider: None,
            config,
        }
    }

    /// Start as a task provider server
    pub async fn start_as_provider(&mut self, socket_path: PathBuf) -> Result<()> {
        let mut provider = TaskServerProvider::new(socket_path, Arc::clone(&self.config));
        provider.start().await?;
        self.server_provider = Some(provider);
        Ok(())
    }

    /// Discover and combine both internal and external tasks
    pub async fn discover_all_tasks(
        &mut self,
        discovery_path: Option<&Path>,
    ) -> Result<Vec<TaskDefinition>> {
        let mut all_tasks = Vec::new();

        // Add internal tasks
        for (name, task_config) in self.config.get_tasks() {
            all_tasks.push(TaskDefinition {
                name: format!("cuenv:{name}"),
                after: task_config.dependencies.clone().unwrap_or_default(),
                description: task_config.description.clone(),
            });
        }

        // Add external tasks from discovery if path provided
        if let Some(path) = discovery_path {
            let external_tasks = self.server_manager.discover_servers(path).await?;
            all_tasks.extend(external_tasks);
        }

        Ok(all_tasks)
    }

    /// Export internal tasks as JSON
    pub fn export_tasks_to_json(&self) -> Result<String> {
        let task_definitions: Vec<TaskDefinition> = self
            .config
            .get_tasks()
            .iter()
            .map(|(name, config)| TaskDefinition {
                name: name.clone(),
                after: config.dependencies.clone().unwrap_or_default(),
                description: config.description.clone(),
            })
            .collect();

        let export = serde_json::json!({
            "tasks": task_definitions
        });

        serde_json::to_string_pretty(&export)
            .map_err(|e| Error::configuration(format!("Failed to serialize tasks to JSON: {e}")))
    }

    /// Shutdown all components
    pub async fn shutdown(&mut self) -> Result<()> {
        // Shutdown external server manager
        self.server_manager.shutdown().await?;

        // Shutdown task provider if running
        if let Some(provider) = self.server_provider.as_mut() {
            provider.shutdown().await?;
        }

        Ok(())
    }
}