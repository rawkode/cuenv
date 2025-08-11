use cuenv_config::TaskConfig;
use std::collections::HashMap;

/// Trait for components that provide task functionality
pub trait TaskSource {
    fn get_tasks(&self) -> &HashMap<String, TaskConfig>;
    fn list_tasks(&self) -> Vec<(String, Option<String>)>;
    fn get_env_vars(&self) -> &HashMap<String, String>;
    fn get_command_capabilities(&self, command: &str) -> Vec<String>;
    fn get_filtered_vars(&self, capabilities: &[String]) -> HashMap<String, String>;
}
