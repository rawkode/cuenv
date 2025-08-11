use super::TaskExecutor;
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
        println!("Cache Statistics:");
        println!("  Hits: {}", stats.hits);
        println!("  Misses: {}", stats.misses);
        println!("  Writes: {}", stats.writes);
        println!("  Errors: {}", stats.errors);
        println!("  Lock contentions: {}", stats.lock_contentions);
        println!("  Total bytes saved: {}", stats.total_bytes_saved);
        if let Some(last_cleanup) = stats.last_cleanup {
            println!("  Last cleanup: {last_cleanup:?}");
        }
        Ok(())
    }

    /// Clean up stale cache entries
    pub fn cleanup_cache(&self, _max_age: Duration) -> Result<(usize, u64)> {
        self.cache_manager.cleanup_stale_entries()?;
        Ok((0, 0)) // Return dummy values for now
    }
}
