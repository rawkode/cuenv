use cuenv_config::{CommandConfig, HookConfig, TaskConfig, TaskNode};
use cuenv_core::{Error, Result};
use cuenv_utils::sync::env::SyncEnv;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::Path;

mod command;
pub mod environment;
mod export;
mod hooks;
mod secrets;
pub mod stubs;
mod task;

pub use stubs::{AccessRestrictions, Shell};
pub use task::TaskSource;

use self::environment::SupervisorMode;

#[derive(Clone)]
pub struct EnvManager {
    original_env: HashMap<String, String>,
    sourced_env: HashMap<String, String>, // Environment from hooks (nix, devenv, etc.)
    cue_vars: HashMap<String, String>,
    cue_vars_metadata: HashMap<String, cuenv_config::VariableMetadata>,
    commands: HashMap<String, CommandConfig>,
    tasks: HashMap<String, TaskConfig>,
    task_nodes: IndexMap<String, TaskNode>, // Preserve task structure and insertion order
    hooks: HashMap<String, HookConfig>,
}

impl EnvManager {
    pub fn new() -> Self {
        Self {
            original_env: HashMap::with_capacity(100),
            sourced_env: HashMap::with_capacity(100),
            cue_vars: HashMap::with_capacity(50),
            cue_vars_metadata: HashMap::with_capacity(50),
            commands: HashMap::with_capacity(20),
            tasks: HashMap::with_capacity(20),
            task_nodes: IndexMap::with_capacity(20),
            hooks: HashMap::with_capacity(4),
        }
    }
}

impl Default for EnvManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvManager {
    pub async fn load_env(&mut self, dir: &Path) -> Result<()> {
        self.load_env_with_options(dir, None, Vec::new(), None, SupervisorMode::Foreground)
            .await
    }

    pub async fn load_env_with_options(
        &mut self,
        dir: &Path,
        environment: Option<String>,
        capabilities: Vec<String>,
        command: Option<&str>,
        mode: SupervisorMode,
    ) -> Result<()> {
        self.save_original_env()?;

        let mut context = environment::LoadEnvironmentContext {
            commands: &mut self.commands,
            tasks: &mut self.tasks,
            task_nodes: &mut self.task_nodes,
            hooks: &mut self.hooks,
            cue_vars: &mut self.cue_vars,
            cue_vars_metadata: &mut self.cue_vars_metadata,
            sourced_env: &mut self.sourced_env,
        };

        environment::load_env_with_options(
            dir,
            environment,
            capabilities,
            command,
            &self.original_env,
            &mut context,
            mode,
        )
        .await?;

        // Execute remaining onEnter hooks after environment variables are set
        environment::execute_on_enter_hooks(&self.hooks)?;
        Ok(())
    }

    pub fn unload_env(&mut self) -> Result<()> {
        environment::unload_env(
            &self.original_env,
            &self.hooks,
            &mut self.cue_vars,
            &mut self.cue_vars_metadata,
        )
    }

    fn save_original_env(&mut self) -> Result<()> {
        self.original_env = SyncEnv::vars()
            .map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?
            .into_iter()
            .collect();
        Ok(())
    }

    pub fn print_env_diff(&self) -> Result<()> {
        export::print_env_diff(&self.original_env)
    }

    pub fn export_for_shell(&self, shell: &str) -> Result<String> {
        export::export_for_shell(&self.original_env, shell)
    }

    pub fn run_command(&self, command: &str, args: &[String]) -> Result<i32> {
        command::run_command(
            command,
            args,
            &self.sourced_env,
            &self.cue_vars,
            &self.original_env,
        )
    }

    /// Run a command with current process environment (includes preload hook variables)
    /// This is used by exec command to ensure preload hook source variables are available
    pub fn run_command_with_current_env(&self, command: &str, args: &[String]) -> Result<i32> {
        // Capture current process environment which includes variables set by preload hooks
        let current_env: std::collections::HashMap<String, String> = std::env::vars().collect();

        command::run_command_direct(
            command,
            args,
            &self.sourced_env,
            &self.cue_vars,
            &current_env, // Use current process env instead of original_env
        )
    }

    /// Run a command with access restrictions in a hermetic environment
    pub fn run_command_with_restrictions(
        &self,
        command: &str,
        args: &[String],
        restrictions: &AccessRestrictions,
    ) -> Result<i32> {
        command::run_command_with_restrictions(
            command,
            args,
            restrictions,
            &self.sourced_env,
            &self.cue_vars,
            &self.original_env,
        )
    }

    /// Get a task by name
    pub fn get_task(&self, task_name: &str) -> Option<&TaskConfig> {
        self.tasks.get(task_name)
    }

    /// List all available tasks with their descriptions
    pub fn list_tasks(&self) -> Vec<(String, Option<String>)> {
        self.tasks
            .iter()
            .map(|(name, config)| (name.clone(), config.description.clone()))
            .collect()
    }

    /// Get all tasks as a HashMap
    pub fn get_tasks(&self) -> &HashMap<String, TaskConfig> {
        &self.tasks
    }

    /// Get CUE environment variables
    pub fn get_cue_vars(&self) -> &HashMap<String, String> {
        &self.cue_vars
    }

    /// Get the capabilities for a specific command
    pub fn get_command_capabilities(&self, command: &str) -> Vec<String> {
        // Extract the base command from the full command string
        let base_command = command.split_whitespace().next().unwrap_or("");

        self.commands
            .get(base_command)
            .and_then(|config| config.capabilities.clone())
            .unwrap_or_default()
    }

    /// Get filtered environment variables based on capabilities
    pub fn get_filtered_vars(&self, capabilities: &[String]) -> HashMap<String, String> {
        self.cue_vars
            .iter()
            .filter(|(key, _)| {
                // Check if this variable should be included based on capabilities
                if let Some(metadata) = self.cue_vars_metadata.get(*key) {
                    if let Some(capability) = &metadata.capability {
                        // Variable has a capability requirement
                        capabilities.contains(capability)
                    } else {
                        // No capability requirement, always include
                        true
                    }
                } else {
                    // No metadata, always include
                    true
                }
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Wait for all preload hooks to complete
    /// This is used by the exec command to ensure the environment is fully prepared
    pub async fn wait_for_preload_hooks(&self) -> Result<()> {
        use cuenv_utils::hooks_status::HooksStatusManager;
        use std::time::Duration;
        use tokio::time::{sleep, timeout};

        let current_dir = std::env::current_dir()
            .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;

        // Create a status manager for the current directory
        let status_manager = HooksStatusManager::new_for_directory(&current_dir).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {e}"))
        })?;

        // Wait for all hooks to complete with a reasonable timeout
        let timeout_duration = Duration::from_secs(300); // 5 minutes timeout

        match timeout(timeout_duration, async {
            loop {
                let status = status_manager.get_current_status();

                // Check if any hooks are still running
                let has_running_hooks = status.hooks.values().any(|hook| {
                    matches!(
                        hook.status,
                        cuenv_utils::hooks_status::HookState::Running
                            | cuenv_utils::hooks_status::HookState::Pending
                    )
                });

                if !has_running_hooks {
                    break;
                }

                // Sleep briefly before checking again
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(cuenv_core::Error::configuration(
                "Timeout waiting for preload hooks to complete",
            )),
        }
    }

    /// Test-only method to populate tasks directly without setting global state
    /// This is marked as doc(hidden) so it doesn't appear in public documentation
    /// but is available for test usage across crates
    #[doc(hidden)]
    pub fn set_tasks_for_testing(
        &mut self,
        tasks: HashMap<String, TaskConfig>,
        task_nodes: IndexMap<String, TaskNode>,
        cue_vars: HashMap<String, String>,
    ) {
        self.tasks = tasks;
        self.task_nodes = task_nodes;
        self.cue_vars = cue_vars;
    }
}

// Implementation modules
mod task_impl;

#[cfg(test)]
mod tests;
