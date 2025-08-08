use crate::config::TaskConfig;
use std::collections::HashMap;

/// Trait for task sources that can provide tasks to the executor
pub trait TaskSource: Send + Sync {
    /// Get all tasks from this source
    fn get_tasks(&self) -> &HashMap<String, TaskConfig>;

    /// List all available tasks with descriptions
    fn list_tasks(&self) -> Vec<(String, Option<String>)>;

    /// Get environment variables
    fn get_env_vars(&self) -> &HashMap<String, String>;

    /// Get command capabilities (for filtering environment variables)
    fn get_command_capabilities(&self, command: &str) -> Vec<String>;

    /// Get filtered environment variables based on capabilities
    fn get_filtered_vars(&self, capabilities: &[String]) -> HashMap<String, String>;

    /// Check if this source supports cross-package references
    fn supports_cross_package(&self) -> bool {
        false
    }

    /// Resolve a task output path (for cross-package dependencies)
    fn resolve_task_output(&self, task_name: &str, output: &str) -> Option<std::path::PathBuf> {
        // Default implementation for single-package sources
        let _ = (task_name, output);
        None
    }
}
