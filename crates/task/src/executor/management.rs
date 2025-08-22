use super::strategies::{process_task_group, TaskGroupExecutionPlan};
use super::unified_dag::UnifiedTaskDAG;
use super::TaskExecutor;
use cuenv_config::{TaskCollection, TaskNode};
use cuenv_core::Result;
use std::collections::HashMap;
use std::time::Duration;

impl TaskExecutor {
    /// List all available tasks
    pub fn list_tasks(&self) -> Vec<(String, Option<String>)> {
        self.env_manager.list_tasks()
    }

    /// Get CUE environment variables
    pub fn get_env_vars(&self) -> &HashMap<String, String> {
        self.env_manager.get_cue_vars()
    }

    /// Get filtered environment variables for a specific task
    pub fn get_task_env_vars(&self, task_name: &str) -> HashMap<String, String> {
        // Get the task config
        let all_tasks = self.env_manager.get_tasks();
        let task_config = match all_tasks.get(task_name) {
            Some(config) => config,
            None => return HashMap::new(),
        };

        // Get the command from the task
        let command = match &task_config.command {
            Some(cmd) => cmd,
            None => return self.env_manager.get_cue_vars().clone(), // No command, return all vars
        };

        // Get capabilities for this command
        let capabilities = self.env_manager.get_command_capabilities(command);

        // Return filtered variables based on capabilities
        self.env_manager.get_filtered_vars(&capabilities)
    }

    /// Clear the task cache
    pub fn clear_cache(&self) -> Result<()> {
        self.cache_manager.clear_cache()
    }

    /// Get cache statistics
    pub fn get_cache_statistics(&self) -> Result<cuenv_cache::manager::CacheStatistics> {
        Ok(self.cache_manager.get_statistics())
    }

    /// Print cache statistics
    pub fn print_cache_statistics(&self) -> Result<()> {
        let stats = self.cache_manager.get_statistics();
        tracing::info!("Cache Statistics:");
        tracing::info!("  Hits: {}", stats.hits);
        tracing::info!("  Misses: {}", stats.misses);
        tracing::info!("  Writes: {}", stats.writes);
        tracing::info!("  Errors: {}", stats.errors);
        tracing::info!("  Lock contentions: {}", stats.lock_contentions);
        tracing::info!("  Total bytes saved: {}", stats.total_bytes_saved);
        if let Some(last_cleanup) = stats.last_cleanup {
            tracing::info!("  Last cleanup: {:?}", last_cleanup);
        }
        Ok(())
    }

    /// Clean up stale cache entries
    pub fn cleanup_cache(&self, _max_age: Duration) -> Result<(usize, u64)> {
        self.cache_manager.cleanup_stale_entries()?;
        Ok((0, 0)) // Return dummy values for now
    }

    /// Process a task group and get its execution plan
    pub fn get_task_group_plan(
        &self,
        group_name: &str,
        tasks: &TaskCollection,
    ) -> Result<TaskGroupExecutionPlan> {
        let plan = process_task_group(group_name, tasks)?;

        // Validate that all tasks in the plan are executable
        for task_info in plan.executable_tasks() {
            // Access all fields to ensure they're used
            log::debug!(
                "Task {} in group {} at path {:?} (barrier: {})",
                task_info.name,
                group_name,
                task_info.group_path,
                task_info.is_barrier
            );

            // Validate task node
            if let TaskNode::Task(config) = &task_info.node {
                log::debug!("Task {} has command: {:?}", task_info.id, config.command);
            }
        }

        Ok(plan)
    }

    /// Build a DAG for specified tasks using the consolidated approach
    pub fn build_dag(&self, task_names: &[String]) -> Result<UnifiedTaskDAG> {
        let all_task_configs = self.env_manager.get_tasks();
        let all_task_nodes = self.env_manager.get_task_nodes();

        // Calculate configuration hash for cache key
        let config_hash = self
            .dag_cache
            .calculate_config_hash(all_task_configs, all_task_nodes);

        // Check cache first
        if let Some(cached_dag) = self.dag_cache.get(task_names, config_hash) {
            log::debug!("Using cached DAG for tasks: {task_names:?}");
            return Ok(cached_dag);
        }

        log::debug!("Building new DAG for tasks: {task_names:?}");

        // Build task definitions using the task builder
        let task_definitions = self
            .task_builder
            .build_tasks_with_nodes(all_task_configs.clone(), all_task_nodes.clone())?;

        let dag = UnifiedTaskDAG::builder()
            .with_task_configs(all_task_configs.clone())
            .with_task_nodes(all_task_nodes.clone())
            .with_task_definitions(task_definitions)
            .build_for_tasks(task_names)?;

        // Store in cache for future use
        if let Err(e) = self.dag_cache.put(task_names, config_hash, dag.clone()) {
            log::warn!("Failed to cache DAG: {e}");
            // Continue execution even if caching fails
        }

        Ok(dag)
    }

    /// Get DAG cache statistics
    pub fn get_dag_cache_stats(&self) -> Result<super::dag_cache::DAGCacheStats> {
        self.dag_cache.stats()
    }

    /// Clear the DAG cache
    pub fn clear_dag_cache(&self) -> Result<()> {
        self.dag_cache.clear()
    }

    /// Clean up expired entries in the DAG cache
    pub fn cleanup_dag_cache(&self) -> Result<usize> {
        self.dag_cache.cleanup_expired()
    }
}
