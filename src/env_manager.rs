use crate::errors::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::command_executor::CommandExecutor;
use crate::cue_parser::{CommandConfig, CueParser, HookConfig, HookType, ParseOptions, TaskConfig};
use crate::hook_manager::HookManager;
use crate::output_filter::OutputFilter;
use crate::platform::{PlatformOps, Shell};
use crate::secrets::SecretManager;
use crate::types::{CommandArguments, EnvironmentVariables};
use async_trait::async_trait;

// Import the platform-specific implementation
#[cfg(unix)]
use crate::platform::UnixPlatform as Platform;
#[cfg(windows)]
use crate::platform::WindowsPlatform as Platform;

pub struct EnvManager {
    original_env: HashMap<String, String>,
    commands: HashMap<String, CommandConfig>,
    tasks: HashMap<String, TaskConfig>,
    hooks: HashMap<String, HookConfig>,
}

impl EnvManager {
    pub fn new() -> Self {
        Self {
            original_env: HashMap::new(),
            commands: HashMap::new(),
            tasks: HashMap::new(),
            hooks: HashMap::new(),
        }
    }
}

impl Default for EnvManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvManager {
    pub fn load_env(&mut self, dir: &Path) -> Result<()> {
        self.load_env_with_options(dir, None, Vec::new(), None)
    }

    pub fn load_env_with_options(
        &mut self,
        dir: &Path,
        environment: Option<String>,
        mut capabilities: Vec<String>,
        command: Option<&str>,
    ) -> Result<()> {
        self.save_original_env();

        // First pass: load package to get command mappings
        let temp_options = ParseOptions {
            environment: environment.clone(),
            capabilities: Vec::new(), // Empty for now to get all commands
        };

        let parse_result = CueParser::eval_package_with_options(dir, "env", &temp_options)?;
        self.commands.extend(parse_result.commands);
        self.tasks.extend(parse_result.tasks);
        self.hooks.extend(parse_result.hooks);

        // If no capabilities were specified, try to infer from the command
        if capabilities.is_empty() {
            if let Some(cmd) = command {
                // Look up the command in our commands configuration
                if let Some(cmd_config) = self.commands.get(cmd) {
                    if let Some(cmd_caps) = &cmd_config.capabilities {
                        log::info!("Inferred capabilities for command '{cmd}': {cmd_caps:?}");
                        capabilities = cmd_caps.clone();
                    }
                }
            }

            if capabilities.is_empty() {
                log::info!(
                    "No capabilities specified or inferred, will load all non-capability-tagged variables"
                );
            }
        }

        // Second pass: load with actual capabilities
        let options = ParseOptions {
            environment,
            capabilities,
        };

        log::info!(
            "Loading CUE package from: {} with env={:?}, capabilities={:?}",
            dir.display(),
            options.environment,
            options.capabilities
        );

        match self.apply_cue_package_with_options(dir, "env", &options) {
            Ok(()) => {
                // Execute onEnter hooks after environment variables are set
                self.execute_on_enter_hooks()?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn unload_env(&self) -> Result<()> {
        let current_env: Vec<(String, String)> = env::vars().collect();

        for (key, _) in current_env {
            if let Some(original_value) = self.original_env.get(&key) {
                env::set_var(&key, original_value);
            } else if !self.original_env.contains_key(&key) {
                env::remove_var(&key);
            }
        }

        Ok(())
    }

    fn save_original_env(&mut self) {
        self.original_env = env::vars().collect();
    }

    fn apply_cue_package_with_options(
        &mut self,
        dir: &Path,
        package_name: &str,
        options: &ParseOptions,
    ) -> Result<()> {
        let parse_result = match CueParser::eval_package_with_options(dir, package_name, options) {
            Ok(result) => result,
            Err(e) => {
                return Err(Error::cue_parse_with_source(
                    dir,
                    format!("Failed to evaluate CUE package: {}", dir.display()),
                    e,
                ));
            }
        };

        // Store commands, tasks and hooks for later use
        self.commands.extend(parse_result.commands);
        self.tasks.extend(parse_result.tasks);
        self.hooks.extend(parse_result.hooks);

        for (key, value) in parse_result.variables {
            let expanded_value = match shellexpand::full(&value) {
                Ok(expanded) => expanded.to_string(),
                Err(e) => {
                    return Err(Error::shell_expansion(
                        &value,
                        format!("Failed to expand value for {key}: {e}"),
                    ));
                }
            };

            log::debug!("Setting {key}={expanded_value}");
            env::set_var(key, expanded_value);
        }

        Ok(())
    }

    pub fn print_env_diff(&self) -> Result<()> {
        let current_env: HashMap<String, String> = env::vars().collect();

        println!("Environment changes:");

        for (key, value) in &current_env {
            if let Some(original) = self.original_env.get(key) {
                if original != value {
                    println!("  {key} (modified): {original} -> {value}");
                }
            } else {
                println!("  {key} (new): {value}");
            }
        }

        for (key, value) in &self.original_env {
            if !current_env.contains_key(key) {
                println!("  {key} (removed): {value}");
            }
        }

        Ok(())
    }

    pub fn export_for_shell(&self, shell: &str) -> Result<String> {
        let current_env: HashMap<String, String> = env::vars().collect();
        let mut output = String::new();

        // Parse shell type
        let shell_type = match shell.parse::<Shell>() {
            Ok(st) => st,
            Err(_) => {
                return Err(Error::unsupported(
                    "shell",
                    format!("Unsupported shell: {shell}"),
                ));
            }
        };

        // Get export format for the shell
        let format = Platform::get_export_format(shell_type);

        // Export new or changed variables
        for (key, value) in &current_env {
            if !self.original_env.contains_key(key) || self.original_env.get(key) != Some(value) {
                output.push_str(&format.format_export(key, value));
                output.push('\n');
            }
        }

        // Unset removed variables
        for key in self.original_env.keys() {
            if !current_env.contains_key(key) {
                output.push_str(&format.format_unset(key));
                output.push('\n');
            }
        }

        Ok(output)
    }

    pub fn run_command(&self, command: &str, args: &[String]) -> Result<i32> {
        // Get the loaded environment variables (only the ones from CUE files)
        let mut env_from_cue = HashMap::new();
        let current_env: HashMap<String, String> = env::vars().collect();

        // Only include variables that were added or modified by CUE
        for (key, value) in &current_env {
            if !self.original_env.contains_key(key) || self.original_env.get(key) != Some(value) {
                env_from_cue.insert(key.clone(), value.clone());
            }
        }

        // Resolve secrets in the environment variables
        let (resolved_env, secret_values) = if cfg!(test) {
            // Skip secret resolution in tests
            use crate::types::SecretValues;
            (env_from_cue, SecretValues::new())
        } else {
            let secret_manager = SecretManager::new();
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(e) => {
                    return Err(Error::configuration(format!(
                        "Failed to create tokio runtime: {e}"
                    )));
                }
            };

            let resolved_secrets =
                match rt.block_on(secret_manager.resolve_secrets(env_from_cue.into())) {
                    Ok(secrets) => secrets,
                    Err(e) => {
                        return Err(Error::secret_resolution(
                            "multiple",
                            format!("Failed to resolve secrets: {e}"),
                        ));
                    }
                };
            (
                resolved_secrets.env_vars.into_inner(),
                resolved_secrets.secret_values,
            )
        };

        // Add minimal required environment variables for basic operation
        let mut final_env = resolved_env;

        // PATH is needed to find executables
        if let Some(path) = self.original_env.get("PATH") {
            final_env.insert("PATH".to_string(), path.clone());
        }

        // Set up platform-specific environment
        Platform::setup_environment(&mut final_env);

        // Ensure HOME directory is available (platform-specific)
        let home_var = Platform::home_env_var();
        if let Some(home_value) = self.original_env.get(home_var) {
            final_env.insert(home_var.to_string(), home_value.clone());
        }

        // Ensure HOME is set on all platforms for compatibility
        if let Some(home) = self.original_env.get("HOME") {
            final_env.insert("HOME".to_string(), home.clone());
        }

        // Create shared secret values for output filtering
        let mut secret_set = HashSet::new();
        for secret in secret_values.iter() {
            secret_set.insert(secret.to_string());
        }
        let secrets = Arc::new(Mutex::new(secret_set));

        // Create and execute the command with only the CUE environment
        let mut cmd = Command::new(command);
        cmd.args(args)
            .env_clear() // Clear all environment variables
            .envs(&final_env) // Set only our CUE-defined vars with resolved secrets
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    format!("Failed to spawn command: {e}"),
                    None,
                ));
            }
        };

        // Set up filtered output streams
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    "Failed to capture stdout".to_string(),
                    None,
                ));
            }
        };
        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    "Failed to capture stderr".to_string(),
                    None,
                ));
            }
        };

        let stdout_secrets = Arc::clone(&secrets);
        let stderr_secrets = Arc::clone(&secrets);

        // Spawn threads to handle output filtering
        let stdout_thread = std::thread::spawn(move || {
            let mut filter = OutputFilter::new(io::stdout(), stdout_secrets);
            io::copy(&mut BufReader::new(stdout), &mut filter)
        });

        let stderr_thread = std::thread::spawn(move || {
            let mut filter = OutputFilter::new(io::stderr(), stderr_secrets);
            io::copy(&mut BufReader::new(stderr), &mut filter)
        });

        // Wait for the process to complete
        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    format!("Failed to wait for command: {e}"),
                    None,
                ));
            }
        };

        // Wait for output threads to complete
        match stdout_thread.join() {
            Ok(result) => match result {
                Ok(_) => {}
                Err(e) => {
                    return Err(Error::command_execution(
                        command,
                        args.to_vec(),
                        format!("Failed to process stdout: {e}"),
                        status.code(),
                    ));
                }
            },
            Err(_) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    "stdout thread panicked".to_string(),
                    status.code(),
                ));
            }
        }

        match stderr_thread.join() {
            Ok(result) => match result {
                Ok(_) => {}
                Err(e) => {
                    return Err(Error::command_execution(
                        command,
                        args.to_vec(),
                        format!("Failed to process stderr: {e}"),
                        status.code(),
                    ));
                }
            },
            Err(_) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    "stderr thread panicked".to_string(),
                    status.code(),
                ));
            }
        }

        Ok(status.code().unwrap_or(1))
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

    fn execute_on_enter_hooks(&self) -> Result<()> {
        // Filter for onEnter hooks
        let on_enter_hooks: Vec<(&String, &HookConfig)> = self
            .hooks
            .iter()
            .filter(|(_, config)| config.hook_type == HookType::OnEnter)
            .collect();

        if on_enter_hooks.is_empty() {
            return Ok(());
        }

        log::info!("Executing {} onEnter hooks", on_enter_hooks.len());

        // Collect current environment variables for hook execution
        let env_vars = self.collect_cue_env_vars();

        // Create runtime for async hook execution
        let rt = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to create tokio runtime for hooks: {e}"
                )));
            }
        };

        // Create a command executor that uses the system's Command
        struct SystemCommandExecutor;

        #[async_trait]
        impl CommandExecutor for SystemCommandExecutor {
            async fn execute(
                &self,
                cmd: &str,
                args: &CommandArguments,
            ) -> Result<std::process::Output> {
                match std::process::Command::new(cmd)
                    .args(args.as_slice())
                    .output()
                {
                    Ok(output) => Ok(output),
                    Err(e) => Err(Error::command_execution(
                        cmd,
                        args.clone().into_inner(),
                        format!("failed to execute command: {e}"),
                        None,
                    )),
                }
            }

            async fn execute_with_env(
                &self,
                cmd: &str,
                args: &CommandArguments,
                env: EnvironmentVariables,
            ) -> Result<std::process::Output> {
                match std::process::Command::new(cmd)
                    .args(args.as_slice())
                    .envs(env.into_inner())
                    .output()
                {
                    Ok(output) => Ok(output),
                    Err(e) => Err(Error::command_execution(
                        cmd,
                        args.clone().into_inner(),
                        format!("failed to execute command with environment: {e}"),
                        None,
                    )),
                }
            }
        }

        // Execute hooks
        let executor = Arc::new(SystemCommandExecutor);
        let hook_manager = match HookManager::new(executor) {
            Ok(hm) => hm,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to create hook manager: {e}"
                )));
            }
        };

        for (name, config) in on_enter_hooks {
            log::debug!("Executing onEnter hook: {name}");
            match rt.block_on(hook_manager.execute_hook(config, &env_vars)) {
                Ok(_) => log::info!("Successfully executed onEnter hook: {name}"),
                Err(e) => {
                    // Log error but continue with other hooks
                    log::error!("Failed to execute onEnter hook '{name}': {e}");
                }
            }
        }

        Ok(())
    }

    fn collect_cue_env_vars(&self) -> HashMap<String, String> {
        let current_env: HashMap<String, String> = env::vars().collect();
        let mut cue_env_vars = HashMap::new();

        // Collect variables that were added or modified by CUE
        for (key, value) in &current_env {
            if !self.original_env.contains_key(key) || self.original_env.get(key) != Some(value) {
                cue_env_vars.insert(key.clone(), value.clone());
            }
        }

        // Add minimal required environment variables for hook execution
        if let Some(path) = self.original_env.get("PATH") {
            cue_env_vars.insert("PATH".to_string(), path.clone());
        }

        // Ensure HOME directory is available
        let home_var = Platform::home_env_var();
        if let Some(home_value) = self.original_env.get(home_var) {
            cue_env_vars.insert(home_var.to_string(), home_value.clone());
        }

        // Ensure HOME is set on all platforms for compatibility
        if let Some(home) = self.original_env.get("HOME") {
            cue_env_vars.insert("HOME".to_string(), home.clone());
        }

        cue_env_vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_and_unload_env() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(
            &env_file,
            r#"package env

env: {
    CUENV_TEST_VAR_UNIQUE: "test_value"
}"#,
        )
        .unwrap();

        let original_value = env::var("CUENV_TEST_VAR_UNIQUE").ok();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();

        assert_eq!(env::var("CUENV_TEST_VAR_UNIQUE").unwrap(), "test_value");

        manager.unload_env().unwrap();

        match original_value {
            Some(val) => assert_eq!(env::var("CUENV_TEST_VAR_UNIQUE").unwrap(), val),
            None => assert!(env::var("CUENV_TEST_VAR_UNIQUE").is_err()),
        }
    }

    #[test]
    fn test_run_command_hermetic() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(
            &env_file,
            r#"package env

env: {
    TEST_FROM_CUE: "cue_value"
    PORT: "8080"
}"#,
        )
        .unwrap();

        // Set a variable that should NOT be passed to the child
        env::set_var("TEST_PARENT_VAR", "should_not_exist");

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();

        // Run a command that checks for our variables
        #[cfg(unix)]
        let (cmd, args) = (
            "sh",
            vec![
                "-c".to_string(),
                "test \"$TEST_FROM_CUE\" = \"cue_value\" && test -z \"$TEST_PARENT_VAR\""
                    .to_string(),
            ],
        );

        #[cfg(windows)]
        let (cmd, args) = ("cmd", vec![
            "/C".to_string(),
            "if \"%TEST_FROM_CUE%\"==\"cue_value\" (if \"%TEST_PARENT_VAR%\"==\"\" exit 0 else exit 1) else exit 1".to_string()
        ]);

        let status = manager.run_command(cmd, &args).unwrap();

        assert_eq!(status, 0, "Command should succeed with correct environment");

        // Clean up
        env::remove_var("TEST_PARENT_VAR");
    }

    #[test]
    fn test_run_command_with_secret_refs() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");

        // Write a CUE file with normal values only
        // We can't test actual secret resolution without mocking the secret managers
        fs::write(
            &env_file,
            r#"package env

env: {
    NORMAL_VAR: "normal-value"
    ANOTHER_VAR: "another-value"
}"#,
        )
        .unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();

        // Run a command that checks the variables
        #[cfg(unix)]
        let (cmd, args) = ("sh", vec![
            "-c".to_string(),
            "test \"$NORMAL_VAR\" = \"normal-value\" && test \"$ANOTHER_VAR\" = \"another-value\"".to_string()
        ]);

        #[cfg(windows)]
        let (cmd, args) = ("cmd", vec![
            "/C".to_string(),
            "if \"%NORMAL_VAR%\"==\"normal-value\" (if \"%ANOTHER_VAR%\"==\"another-value\" exit 0 else exit 1) else exit 1".to_string()
        ]);

        let status = manager.run_command(cmd, &args).unwrap();

        assert_eq!(status, 0, "Command should succeed with all variables set");
    }

    #[test]
    fn test_run_command_preserves_path_and_home() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(
            &env_file,
            r#"package env

env: {
    TEST_VAR: "test"
}"#,
        )
        .unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();

        // Run a command that checks PATH and HOME are preserved
        #[cfg(unix)]
        let (cmd, args) = (
            "sh",
            vec![
                "-c".to_string(),
                "test -n \"$PATH\" && test -n \"$HOME\"".to_string(),
            ],
        );

        #[cfg(windows)]
        let (cmd, args) = (
            "cmd",
            vec![
                "/C".to_string(),
                "if defined PATH (if defined HOME exit 0 else exit 1) else exit 1".to_string(),
            ],
        );

        let status = manager.run_command(cmd, &args).unwrap();

        assert_eq!(status, 0, "PATH and HOME should be preserved");
    }
}
