use cuenv_config::{TaskConfig, TaskNode};
use std::collections::HashMap;

use super::task::TaskSource;
use super::EnvManager;

// Implement TaskSource trait for EnvManager
impl TaskSource for EnvManager {
    fn get_tasks(&self) -> &HashMap<String, TaskConfig> {
        &self.tasks
    }

    fn list_tasks(&self) -> Vec<(String, Option<String>)> {
        self.list_tasks()
    }

    fn get_env_vars(&self) -> &HashMap<String, String> {
        self.get_cue_vars()
    }

    fn get_command_capabilities(&self, command: &str) -> Vec<String> {
        self.get_command_capabilities(command)
    }

    fn get_filtered_vars(&self, capabilities: &[String]) -> HashMap<String, String> {
        self.get_filtered_vars(capabilities)
    }
}

impl EnvManager {
    /// Get task nodes (preserving group structure)
    pub fn get_task_nodes(&self) -> &HashMap<String, TaskNode> {
        &self.task_nodes
    }
}
