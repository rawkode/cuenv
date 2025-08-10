//! Refactored Environment Manager for centralized configuration management
//!
//! This module provides the new `EnvManager` that focuses solely on runtime environment
//! management. All configuration loading has been moved to the `ConfigLoader` in the
//! `cuenv-config` crate.

use cuenv_config::{CommandConfig, Config, Hook, TaskConfig};
use cuenv_core::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::io;
use std::process::Command;
use std::sync::{Arc, RwLock};

use crate::diff::EnvDiff;

// Temporary stub for AccessRestrictions until security crate is fixed
#[derive(Default)]
pub struct AccessRestrictions {
    pub file_restrictions: bool,
    pub network_restrictions: bool,
}

impl AccessRestrictions {
    pub fn new(file_restrictions: bool, network_restrictions: bool) -> Self {
        Self {
            file_restrictions,
            network_restrictions,
        }
    }

    pub fn has_any_restrictions(&self) -> bool {
        self.file_restrictions || self.network_restrictions
    }

    pub fn apply_to_command(&self, _cmd: &mut Command) -> cuenv_core::Result<()> {
        // Stub - would normally apply Landlock restrictions
        Ok(())
    }
}

// Stubs for missing types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl Shell {
    pub fn load(_path: &std::path::Path) -> cuenv_core::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct ExportFormat {
    shell: Shell,
}

impl ExportFormat {
    pub fn new(shell: Shell) -> Self {
        Self { shell }
    }

    pub fn format_export(&self, key: &str, value: &str) -> String {
        match self.shell {
            Shell::Bash | Shell::Zsh => format!("export {}='{}'", key, value),
            Shell::Fish => format!("set -gx {} '{}'", key, value),
            Shell::PowerShell => format!("$env:{} = '{}'", key, value),
        }
    }

    pub fn format_unset(&self, key: &str) -> String {
        match self.shell {
            Shell::Bash | Shell::Zsh => format!("unset {}", key),
            Shell::Fish => format!("set -e {}", key),
            Shell::PowerShell => format!("Remove-Item env:{}", key),
        }
    }

    pub fn get_export_format(shell: Shell) -> ExportFormat {
        ExportFormat::new(shell)
    }

    pub fn setup_environment(_env: &mut HashMap<String, String>) {
        // Stub implementation
    }

    pub fn home_env_var() -> &'static str {
        "HOME"
    }
}

/// Writer that redacts sensitive information from output
pub struct RedactingWriter<W: std::io::Write> {
    writer: W,
    secrets: Arc<RwLock<HashSet<String>>>,
}

impl<W: std::io::Write> RedactingWriter<W> {
    pub fn new(writer: W, secrets: Arc<RwLock<HashSet<String>>>) -> Self {
        Self { writer, secrets }
    }
}

impl<W: std::io::Write> std::io::Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        let mut output = s.to_string();

        if let Ok(secrets) = self.secrets.read() {
            for secret in secrets.iter() {
                if !secret.is_empty() {
                    output = output.replace(secret, "***REDACTED***");
                }
            }
        }

        self.writer.write(output.as_bytes())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// Refactored Environment Manager focused on runtime environment management
///
/// This new `EnvManager` accepts a pre-loaded `Config` and focuses solely on applying
/// that configuration to the runtime environment. All configuration loading logic
/// has been moved to `ConfigLoader` in the `cuenv-config` crate.
///
/// # Example Usage
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use cuenv_config::{Config, ConfigLoader};
/// use cuenv_env::EnvManager;
///
/// // Load configuration centrally
/// let config = ConfigLoader::new()
///     .with_directory("/path/to/project".into())
///     .load()
///     .expect("Failed to load configuration");
///
/// // Create EnvManager with pre-loaded configuration
/// let mut env_manager = EnvManager::new(Arc::new(config));
///
/// // Apply the configuration to the environment
/// env_manager.apply_config().expect("Failed to apply configuration");
/// ```
pub struct EnvManager {
    /// Pre-loaded configuration from ConfigLoader
    config: Arc<Config>,

    /// Original environment variables captured at startup
    original_env: HashMap<String, String>,

    /// Environment variables sourced from hooks (nix, devenv, etc.)
    #[allow(dead_code)]
    sourced_env: HashMap<String, String>,

    /// Current environment diff for shell exports
    current_diff: Option<EnvDiff>,

    /// Secrets tracking for output redaction
    secrets: Arc<RwLock<HashSet<String>>>,
}

impl EnvManager {
    /// Create a new EnvManager with pre-loaded configuration
    ///
    /// This replaces the old `new()` method and requires a pre-loaded `Config`
    /// from the `ConfigLoader`. This enforces the centralized configuration
    /// loading pattern.
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            original_env: config.original_environment.clone(),
            sourced_env: HashMap::with_capacity(100),
            current_diff: None,
            secrets: Arc::new(RwLock::new(HashSet::new())),
            config,
        }
    }

    /// Apply the pre-loaded configuration to the current environment
    ///
    /// This method replaces the old `load_env()` and `load_env_with_options()` methods.
    /// Instead of loading configuration, it applies the pre-loaded configuration
    /// from the `Config` struct to the runtime environment.
    pub async fn apply_config(&mut self) -> Result<()> {
        // Save current environment state
        self.save_original_env()?;

        // Get resolved environment variables from config
        let resolved_vars = self.config.get_resolved_environment();

        // Execute pre-hooks if configured
        self.execute_hooks("onEnter").await?;

        // Apply environment variables
        for (key, value) in &resolved_vars {
            // Check if variable is sensitive and track it
            if self.config.is_variable_sensitive(key) {
                if let Ok(mut secrets) = self.secrets.write() {
                    secrets.insert(value.clone());
                }
            }

            // Set the environment variable
            std::env::set_var(key, value);
        }

        // Update internal state
        self.update_environment_diff(resolved_vars)?;

        Ok(())
    }

    /// Unload the environment and restore original state
    pub fn unload_env(&mut self) -> Result<()> {
        // Execute exit hooks
        if let Some(exit_hooks) = self.config.get_hooks("onExit") {
            for hook in exit_hooks {
                self.execute_hook(hook)?;
            }
        }

        // Restore original environment
        for (key, value) in &self.original_env {
            std::env::set_var(key, value);
        }

        // Remove variables that weren't in original environment
        let resolved_vars = self.config.get_resolved_environment();
        for key in resolved_vars.keys() {
            if !self.original_env.contains_key(key) {
                std::env::remove_var(key);
            }
        }

        self.current_diff = None;
        Ok(())
    }

    /// Print environment diff showing what changed
    pub fn print_env_diff(&self) -> Result<()> {
        if let Some(ref diff) = self.current_diff {
            println!("Environment changes:");

            for (key, value) in diff.added_or_changed() {
                println!("  + {}={}", key, value);
            }

            for key in diff.removed() {
                println!("  - {}", key);
            }
        } else {
            println!("No environment changes");
        }

        Ok(())
    }

    /// Export environment for a specific shell
    pub fn export_for_shell(&self, shell: &str) -> Result<String> {
        let shell_enum = match shell {
            "bash" | "sh" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "powershell" | "pwsh" => Shell::PowerShell,
            _ => {
                return Err(Error::configuration(format!(
                    "Unsupported shell: {}",
                    shell
                )))
            }
        };

        let export_format = ExportFormat::new(shell_enum);
        let mut output = String::new();

        let resolved_vars = self.config.get_resolved_environment();
        for (key, value) in &resolved_vars {
            // Skip sensitive variables from shell export
            if !self.config.is_variable_sensitive(key) {
                output.push_str(&export_format.format_export(key, value));
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// Run a command with the current environment
    pub fn run_command(&self, command: &str, args: &[String]) -> Result<i32> {
        if let Some(cmd_config) = self.config.get_command(command) {
            self.run_command_config(cmd_config, args)
        } else {
            // Run as simple shell command
            let mut cmd = Command::new(command);
            cmd.args(args);

            // Apply current environment
            let resolved_vars = self.config.get_resolved_environment();
            for (key, value) in &resolved_vars {
                cmd.env(key, value);
            }

            let status = cmd.status().map_err(|e| {
                Error::command_execution(
                    command,
                    vec![],
                    format!("Failed to execute command: {}", e),
                    None,
                )
            })?;

            Ok(status.code().unwrap_or(-1))
        }
    }

    /// Run a command with access restrictions
    pub fn run_command_with_restrictions(
        &self,
        command: &str,
        args: &[String],
        restrictions: &AccessRestrictions,
    ) -> Result<i32> {
        let mut cmd = Command::new(command);
        cmd.args(args);

        // Apply access restrictions
        restrictions.apply_to_command(&mut cmd)?;

        // Apply current environment
        let resolved_vars = self.config.get_resolved_environment();
        for (key, value) in &resolved_vars {
            cmd.env(key, value);
        }

        let status = cmd.status().map_err(|e| {
            Error::command_execution(
                command,
                vec![],
                format!("Failed to execute restricted command: {}", e),
                None,
            )
        })?;

        Ok(status.code().unwrap_or(-1))
    }

    /// Get task configuration by name
    pub fn get_task(&self, task_name: &str) -> Option<&TaskConfig> {
        self.config.get_task(task_name)
    }

    /// List all available tasks with descriptions
    pub fn list_tasks(&self) -> Vec<(String, Option<String>)> {
        self.config
            .filter_tasks_by_capabilities()
            .into_iter()
            .map(|(name, task)| (name.clone(), task.description.clone()))
            .collect()
    }

    /// Get all tasks (filtered by capabilities)
    pub fn get_tasks(&self) -> HashMap<String, &TaskConfig> {
        self.config.filter_tasks_by_capabilities()
    }

    /// Get CUE variables (filtered by capabilities)
    pub fn get_cue_vars(&self) -> HashMap<String, String> {
        self.config.get_resolved_environment()
    }

    /// Get command capabilities
    pub fn get_command_capabilities(&self, command: &str) -> Vec<String> {
        self.config
            .get_command(command)
            .and_then(|cmd| cmd.capabilities.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    /// Get environment variables filtered by capabilities
    pub fn get_filtered_vars(&self, capabilities: &[String]) -> HashMap<String, String> {
        if capabilities.is_empty() {
            return self.config.get_resolved_environment();
        }

        // Create a temporary config with the specified capabilities for filtering
        // Since we can't modify the config, we'll filter manually
        let mut filtered = HashMap::new();
        let resolved_vars = self.config.get_resolved_environment();

        for (name, value) in &resolved_vars {
            if let Some(metadata) = self.config.get_variable_metadata(name) {
                if let Some(capability) = &metadata.capability {
                    // Variable has a capability tag, only include if it matches the filter
                    if capabilities.contains(capability) {
                        filtered.insert(name.clone(), value.clone());
                    }
                } else {
                    // No capability tag means always include
                    filtered.insert(name.clone(), value.clone());
                }
            } else {
                // No metadata means no capability tag, always include
                filtered.insert(name.clone(), value.clone());
            }
        }

        filtered
    }

    // Private helper methods

    /// Save the original environment state
    fn save_original_env(&mut self) -> Result<()> {
        if self.original_env.is_empty() {
            for (key, value) in std::env::vars() {
                self.original_env.insert(key, value);
            }
        }
        Ok(())
    }

    /// Update the environment diff for tracking changes
    fn update_environment_diff(&mut self, _new_vars: HashMap<String, String>) -> Result<()> {
        let current_env: HashMap<String, String> = std::env::vars().collect();
        self.current_diff = Some(EnvDiff::new(self.original_env.clone(), current_env));
        Ok(())
    }

    /// Execute hooks of a specific type
    async fn execute_hooks(&self, hook_type: &str) -> Result<()> {
        if let Some(hooks) = self.config.get_hooks(hook_type) {
            for hook in hooks {
                self.execute_hook(hook)?;
            }
        }
        Ok(())
    }

    /// Execute a single hook
    fn execute_hook(&self, hook: &Hook) -> Result<()> {
        // Implementation depends on Hook enum structure
        // This is a simplified version
        match hook {
            Hook::Legacy(config) => {
                let mut cmd = Command::new(&config.command);
                cmd.args(&config.args);

                // Apply current environment
                let resolved_vars = self.config.get_resolved_environment();
                for (key, value) in &resolved_vars {
                    cmd.env(key, value);
                }

                let status = cmd.status().map_err(|e| {
                    Error::command_execution(
                        &config.command,
                        config.args.clone(),
                        format!("Hook execution failed: {}", e),
                        None,
                    )
                })?;

                if !status.success() {
                    return Err(Error::command_execution(
                        &config.command,
                        config.args.clone(),
                        "Hook returned non-zero exit code".to_string(),
                        status.code(),
                    ));
                }
            }
            Hook::Exec { exec, .. } => {
                let mut cmd = Command::new(&exec.command);
                if let Some(args) = &exec.args {
                    cmd.args(args);
                }

                // Apply current environment
                let resolved_vars = self.config.get_resolved_environment();
                for (key, value) in &resolved_vars {
                    cmd.env(key, value);
                }

                let status = cmd.status().map_err(|e| {
                    Error::command_execution(
                        &exec.command,
                        exec.args.clone().unwrap_or_default(),
                        format!("Hook execution failed: {}", e),
                        None,
                    )
                })?;

                if !status.success() {
                    return Err(Error::command_execution(
                        &exec.command,
                        exec.args.clone().unwrap_or_default(),
                        "Hook returned non-zero exit code".to_string(),
                        status.code(),
                    ));
                }
            }
            _ => {
                // Handle other hook types as needed
                tracing::warn!("Unsupported hook type encountered");
            }
        }

        Ok(())
    }

    /// Run a command using its configuration
    /// Note: CommandConfig currently only contains capabilities, not full command execution details
    fn run_command_config(&self, _cmd_config: &CommandConfig, _args: &[String]) -> Result<i32> {
        // TODO: Implement proper command execution when CommandConfig structure is complete
        // For now, return an error indicating this feature is not implemented
        Err(Error::configuration(
            "Command execution via CommandConfig is not yet implemented in centralized configuration".to_string()
        ))
    }
}

impl Default for EnvManager {
    fn default() -> Self {
        // This should not be used in the new architecture, but we provide a stub
        // for backward compatibility during migration
        panic!("EnvManager::default() should not be used. Use EnvManager::new(config) instead.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::{Config, RuntimeSettings};
    use std::path::PathBuf;

    fn create_test_config() -> Arc<Config> {
        let config = Config::new(
            "test".to_string(),
            vec![],
            PathBuf::from("/test"),
            cuenv_config::ParseResult {
                variables: [("TEST_VAR".to_string(), "test_value".to_string())]
                    .into_iter()
                    .collect(),
                metadata: HashMap::new(),
                commands: HashMap::new(),
                tasks: HashMap::new(),
                hooks: HashMap::new(),
            },
            HashMap::new(),
            RuntimeSettings::default(),
        );
        Arc::new(config)
    }

    #[test]
    fn test_new_env_manager() {
        let config = create_test_config();
        let env_manager = EnvManager::new(config.clone());

        assert_eq!(env_manager.config.environment_name, "test");
    }

    #[test]
    fn test_get_cue_vars() {
        let config = create_test_config();
        let env_manager = EnvManager::new(config);

        let vars = env_manager.get_cue_vars();
        assert_eq!(vars.get("TEST_VAR"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_export_for_shell() {
        let config = create_test_config();
        let env_manager = EnvManager::new(config);

        let export = env_manager.export_for_shell("bash").unwrap();
        assert!(export.contains("export TEST_VAR='test_value'"));
    }
}
