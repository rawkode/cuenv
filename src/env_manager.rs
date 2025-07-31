use crate::core::errors::{Error, Result};
use crate::utils::sync::env::SyncEnv;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};

use crate::access_restrictions::AccessRestrictions;
<<<<<<< HEAD
use crate::cue_parser::{CommandConfig, CueParser, ExecConfig, Hook, ParseOptions, TaskConfig};
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
use crate::command_executor::CommandExecutor;
use crate::cue_parser::{CommandConfig, CueParser, HookConfig, HookType, ParseOptions, TaskConfig};
=======
use crate::command_executor::CommandExecutor;
use crate::core::types::{CommandArguments, EnvironmentVariables};
use crate::cue_parser::{CommandConfig, CueParser, HookConfig, HookType, ParseOptions, TaskConfig};
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
use crate::env_diff::EnvDiff;
use crate::file_times::FileTimes;
use crate::output_filter::OutputFilter;
use crate::platform::{PlatformOps, Shell};
use crate::secrets::SecretManager;
use crate::state::StateManager;
<<<<<<< HEAD
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
use crate::types::{CommandArguments, EnvironmentVariables};
use async_trait::async_trait;
=======
use async_trait::async_trait;
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)

// Import the platform-specific implementation
#[cfg(unix)]
use crate::platform::UnixPlatform as Platform;
#[cfg(windows)]
use crate::platform::WindowsPlatform as Platform;

#[derive(Clone)]
pub struct EnvManager {
    original_env: HashMap<String, String>,
    cue_vars: HashMap<String, String>,
    cue_vars_metadata: HashMap<String, crate::cue_parser::VariableMetadata>,
    commands: HashMap<String, CommandConfig>,
    tasks: HashMap<String, TaskConfig>,
    hooks: HashMap<String, Vec<Hook>>,
}

impl EnvManager {
    pub fn new() -> Self {
        Self {
            // Pre-allocate with reasonable initial capacities to reduce rehashing
            original_env: HashMap::with_capacity(100), // Environment typically has many vars
            cue_vars: HashMap::with_capacity(50),      // CUE vars are usually fewer
            cue_vars_metadata: HashMap::with_capacity(50), // Metadata for each var
            commands: HashMap::with_capacity(20),      // Commands are limited
            tasks: HashMap::with_capacity(20),         // Tasks are also limited
            hooks: HashMap::with_capacity(4),          // Usually only a few hooks
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
        self.load_env_with_options(dir, None, Vec::new(), None)
            .await
    }

    pub async fn load_env_with_options(
        &mut self,
        dir: &Path,
        environment: Option<String>,
        mut capabilities: Vec<String>,
        command: Option<&str>,
    ) -> Result<()> {
        self.save_original_env()?;

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
                        tracing::info!(
                            command = %cmd,
                            capabilities = ?cmd_caps,
                            "Inferred capabilities for command"
                        );
                        capabilities = cmd_caps.clone();
                    }
                }
            }

            if capabilities.is_empty() {
                tracing::info!(
                    "No capabilities specified or inferred, will load all non-capability-tagged variables"
                );
            }
        }

        // Second pass: load with actual capabilities
        let options = ParseOptions {
            environment,
            capabilities,
        };

        tracing::info!(
            path = %dir.display(),
            environment = ?options.environment,
            capabilities = ?options.capabilities,
            "Loading CUE package"
        );

        // First, parse CUE package to get hooks and initial environment
        let parse_result = match CueParser::eval_package_with_options(dir, "env", &options) {
            Ok(result) => result,
            Err(e) => {
                return Err(Error::cue_parse_with_source(
                    dir,
                    format!("Failed to evaluate CUE package: {}", dir.display()),
                    e,
                ));
            }
        };

        // Store commands, tasks and hooks
        self.commands.extend(parse_result.commands.clone());
        self.tasks.extend(parse_result.tasks.clone());
        self.hooks.extend(parse_result.hooks.clone());

        // Execute sourcing hooks first to capture additional environment variables
        let mut sourced_env_vars = HashMap::new();
        if let Some(on_enter_hooks) = parse_result.hooks.get("onEnter") {
            for hook in on_enter_hooks {
                // Check if this hook should be sourced
                if self.should_source_hook(hook) {
                    match self.execute_sourcing_hook(hook).await {
                        Ok(vars) => {
                            log::info!("Sourced {} environment variables from hook", vars.len());
                            sourced_env_vars.extend(vars);
                        }
                        Err(e) => {
                            log::error!("Failed to execute sourcing hook: {e}");
                            // Continue with other hooks instead of failing completely
                        }
                    }
                }
            }
        }

        // Merge CUE variables with sourced variables (CUE takes precedence)
        let mut merged_variables = sourced_env_vars;
        merged_variables.extend(parse_result.variables);

        // Apply the merged environment
        match self
            .apply_merged_environment(dir, merged_variables, &options)
            .await
        {
            Ok(()) => {
                // Execute remaining onEnter hooks after environment variables are set
                self.execute_on_enter_hooks().await?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn unload_env(&mut self) -> Result<()> {
        // Execute onExit hooks before unloading environment
        if let Some(exit_hooks) = self.hooks.get("onExit") {
            if !exit_hooks.is_empty() {
                log::info!("Executing {} onExit hooks", exit_hooks.len());

<<<<<<< HEAD
                // Execute onExit hooks using async runtime
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
        if !exit_hooks.is_empty() {
            log::info!("Executing {} onExit hooks", exit_hooks.len());
            // Use run_async to avoid creating a new runtime if already in one
            // This is safe because unload_env is called from a sync context (main.rs)
            // but the hook execution itself is async.
            // We don't need a separate runtime here, as hook_manager.execute_hook is async.
            // The block_on will be handled by run_async if needed.

            // Create command executor and hook manager
            let executor = Arc::new(crate::command_executor::SystemCommandExecutor::new());
            let hook_manager = match HookManager::new(executor) {
                Ok(hm) => hm,
                Err(e) => {
                    log::error!("Failed to create hook manager: {e}");
                    return Err(Error::configuration(format!(
                        "Failed to create hook manager: {e}"
                    )));
                }
            };

            // Get current environment variables for hook execution
            let current_env_vars: HashMap<String, String> = SyncEnv::vars()
                .map_err(|e| Error::Configuration {
                    message: format!("Failed to get environment variables: {e}"),
                })?
                .into_iter()
                .collect();

            for (name, config) in exit_hooks {
                log::debug!("Executing onExit hook: {name}");
=======
        if !exit_hooks.is_empty() {
            tracing::info!(
                count = %exit_hooks.len(),
                "Executing onExit hooks"
            );
            // Use run_async to avoid creating a new runtime if already in one
            // This is safe because unload_env is called from a sync context (main.rs)
            // but the hook execution itself is async.
            // We don't need a separate runtime here, as hook_manager.execute_hook is async.
            // The block_on will be handled by run_async if needed.

            // Create command executor and hook manager
            let executor = Arc::new(crate::command_executor::SystemCommandExecutor::new());
            let hook_manager = match HookManager::new(executor) {
                Ok(hm) => hm,
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "Failed to create hook manager"
                    );
                    return Err(Error::configuration(format!(
                        "Failed to create hook manager: {e}"
                    )));
                }
            };

            // Get current environment variables for hook execution
            let current_env_vars: HashMap<String, String> = SyncEnv::vars()
                .map_err(|e| Error::Configuration {
                    message: format!("Failed to get environment variables: {e}"),
                })?
                .into_iter()
                .collect();

            for (name, config) in exit_hooks {
                tracing::debug!(
                    hook_name = %name,
                    "Executing onExit hook"
                );
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                match crate::async_runtime::run_async(async {
                    for hook in exit_hooks {
                        if let Err(e) = self.execute_non_sourcing_hook(hook).await {
                            log::error!("Failed to execute onExit hook: {e}");
                            // Continue with other hooks instead of failing completely
                        }
                    }
                    Ok::<(), Error>(())
                }) {
<<<<<<< HEAD
                    Ok(()) => {}
                    Err(e) => {
                        log::error!("Failed to execute onExit hooks: {e}");
                        return Err(e);
                    }
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                    Ok(_) => log::info!("Successfully executed onExit hook: {name}"),
                    Err(e) => log::error!("Failed to execute onExit hook {name}: {e}"),
=======
                    Ok(_) => tracing::info!(
                        hook_name = %name,
                        "Successfully executed onExit hook"
                    ),
                    Err(e) => tracing::error!(
                        hook_name = %name,
                        error = %e,
                        "Failed to execute onExit hook"
                    ),
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                }
            }
        }

        // Restore original environment
        let current_env: Vec<(String, String)> =
            SyncEnv::vars().map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?;

        for (key, _) in current_env {
            if let Some(original_value) = self.original_env.get(&key) {
                SyncEnv::set_var(&key, original_value).map_err(|e| Error::Configuration {
                    message: format!("Failed to get environment variables: {e}"),
                })?;
            } else if !self.original_env.contains_key(&key) {
                SyncEnv::remove_var(&key).map_err(|e| Error::Configuration {
                    message: format!("Failed to get environment variables: {e}"),
                })?;
            }
        }

        // Clear CUE vars and metadata
        self.cue_vars.clear();
        self.cue_vars_metadata.clear();

        Ok(())
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

    #[allow(dead_code)]
    async fn apply_cue_package_with_options(
        &mut self,
        dir: &Path,
        package_name: &str,
        options: &ParseOptions,
    ) -> Result<()> {
        // Only allow loading the "env" package
        if package_name != "env" {
            return Err(Error::configuration(format!(
                "Only 'env' package is supported, got '{package_name}'. Please ensure your .cue files use 'package env'"
            )));
        }
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

        // Store variable metadata
        self.cue_vars_metadata.clear();
        self.cue_vars_metadata.extend(parse_result.metadata);

        // Build the new environment
        let mut new_env = self.original_env.clone();
        self.cue_vars.clear();
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

            tracing::debug!(
                key = %key,
                value = %expanded_value,
                "Setting environment variable"
            );
            new_env.insert(key.clone(), expanded_value.clone());
            self.cue_vars.insert(key.clone(), expanded_value.clone());
            SyncEnv::set_var(key, expanded_value).map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?;
        }

        // Create environment diff
        let diff = EnvDiff::new(self.original_env.clone(), new_env);

        // Create file watches
        let mut watches = FileTimes::new();
        let env_cue = dir.join("env.cue");
        if env_cue.exists() {
            watches.watch(&env_cue);
        }

        // Save state
        StateManager::load(
            dir,
            &env_cue,
            options.environment.as_deref(),
            &options.capabilities,
            &diff,
            &watches,
        )
        .await
        .map_err(|e| Error::configuration(format!("Failed to save state: {e}")))?;

        Ok(())
    }

    pub fn print_env_diff(&self) -> Result<()> {
        let current_env: HashMap<String, String> = SyncEnv::vars()
            .map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?
            .into_iter()
            .collect();

        // Emit structured events for environment changes while maintaining user output
        let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());

        if is_tty {
            // In TTY mode, emit structured events for the tree view
            tracing::info!("Environment changes detected");

            for (key, value) in &current_env {
                if let Some(original) = self.original_env.get(key) {
                    if original != value {
                        tracing::info!(
                            key = %key,
                            old_value = %original,
                            new_value = %value,
                            change_type = "modified",
                            "Environment variable modified"
                        );
                    }
                } else {
                    tracing::info!(
                        key = %key,
                        value = %value,
                        change_type = "new",
                        "Environment variable added"
                    );
                }
            }

            for (key, value) in &self.original_env {
                if !current_env.contains_key(key) {
                    tracing::info!(
                        key = %key,
                        value = %value,
                        change_type = "removed",
                        "Environment variable removed"
                    );
                }
            }
        } else {
            // In non-TTY mode, maintain original output format
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
        }

        Ok(())
    }

    pub fn export_for_shell(&self, shell: &str) -> Result<String> {
        let current_env: HashMap<String, String> = SyncEnv::vars()
            .map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?
            .into_iter()
            .collect();
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
            if !self.original_env.contains_key(key as &str)
                || self.original_env.get(key as &str) != Some(value)
            {
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
        let env_from_cue = self.cue_vars.clone();

        // Resolve secrets in the environment variables
        let (resolved_env, secret_values) = if cfg!(test) {
            // Skip secret resolution in tests
            use crate::core::types::SecretValues;
            (env_from_cue, SecretValues::new())
        } else {
            let secret_manager = SecretManager::new();

            // Use futures::executor::block_on which works in more contexts
            let resolved_secrets = match futures::executor::block_on(async {
                secret_manager.resolve_secrets(env_from_cue.into()).await
            }) {
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
        let secrets = Arc::new(RwLock::new(secret_set));

        // Create and execute the command with only the CUE environment
        let mut cmd = Command::new(command);
        cmd.args(args)
            .env_clear() // Clear all environment variables
            .envs(&final_env) // Set only our CUE-defined vars with resolved secrets
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Configure process group for better cleanup on Unix
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

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

    /// Run a command with access restrictions in a hermetic environment
    pub fn run_command_with_restrictions(
        &self,
        command: &str,
        args: &[String],
        restrictions: &AccessRestrictions,
    ) -> Result<i32> {
        // Get the loaded environment variables (only the ones from CUE files)
        let env_from_cue = self.cue_vars.clone();

        // Resolve secrets in the environment variables
        let (resolved_env, secret_values) = if cfg!(test) {
            // Skip secret resolution in tests
            use crate::core::types::SecretValues;
            (env_from_cue, SecretValues::new())
        } else {
            let secret_manager = SecretManager::new();

            // Use futures::executor::block_on which works in more contexts
            let resolved_secrets = match futures::executor::block_on(async {
                secret_manager.resolve_secrets(env_from_cue.into()).await
            }) {
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
        let secrets = Arc::new(RwLock::new(secret_set));

        // Create and execute the command with only the CUE environment
        let mut cmd = Command::new(command);
        cmd.args(args)
            .env_clear() // Clear all environment variables
            .envs(&final_env) // Set only our CUE-defined vars with resolved secrets
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply access restrictions before spawning the process
        restrictions.apply_to_command(&mut cmd)?;

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
                    "stdout processing thread panicked".to_string(),
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
                    "stderr processing thread panicked".to_string(),
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

<<<<<<< HEAD
    async fn execute_on_enter_hooks(&self) -> Result<()> {
        // Execute onEnter hooks that are NOT sourcing hooks (those were already executed)
        if let Some(on_enter_hooks) = self.hooks.get("onEnter") {
            let non_sourcing_hooks: Vec<&Hook> = on_enter_hooks
                .iter()
                .filter(|hook| !self.should_source_hook(hook))
                .collect();
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
    fn execute_on_enter_hooks(&self) -> Result<()> {
        // Filter for onEnter hooks
        let on_enter_hooks: Vec<(&String, &HookConfig)> = self
            .hooks
            .iter()
            .filter(|(_, config)| config.hook_type == HookType::OnEnter)
            .collect();
=======
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

    fn execute_on_enter_hooks(&self) -> Result<()> {
        // Filter for onEnter hooks
        let on_enter_hooks: Vec<(&String, &HookConfig)> = self
            .hooks
            .iter()
            .filter(|(_, config)| config.hook_type == HookType::OnEnter)
            .collect();
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)

            if non_sourcing_hooks.is_empty() {
                return Ok(());
            }

            log::info!("Executing {} onEnter hooks", non_sourcing_hooks.len());

            for hook in non_sourcing_hooks {
                if let Err(e) = self.execute_non_sourcing_hook(hook).await {
                    log::error!("Failed to execute onEnter hook: {e}");
                    // Continue with other hooks instead of failing completely
                }
            }
        }

<<<<<<< HEAD
        Ok(())
    }

    /// Check if a hook should be sourced
    fn should_source_hook(&self, hook: &Hook) -> bool {
        match hook {
            Hook::Legacy(hook_config) => hook_config.source.unwrap_or(false),
            Hook::Exec { exec, .. } => exec.source.unwrap_or(false),
            Hook::NixFlake { exec, .. } => exec.source.unwrap_or(false),
            Hook::Devenv { exec, .. } => exec.source.unwrap_or(false),
        }
    }

    /// Execute sourcing hooks to capture environment variables from new Hook enum
    async fn execute_sourcing_hook(&self, hook: &Hook) -> Result<HashMap<String, String>> {
        match hook {
            Hook::Legacy(hook_config) => {
                // Convert HookConfig to ExecConfig for legacy support
                let exec_config = ExecConfig {
                    command: hook_config.command.clone(),
                    args: Some(hook_config.args.clone()),
                    dir: None,
                    inputs: None,
                    source: hook_config.source,
                    constraints: hook_config.constraints.clone(),
                };
                self.execute_exec_sourcing_hook(&exec_config).await
            }
            Hook::Exec { exec, .. } => self.execute_exec_sourcing_hook(exec).await,
            Hook::NixFlake { exec, flake, .. } => {
                self.execute_nix_flake_sourcing_hook(exec, flake).await
            }
            Hook::Devenv { exec, devenv, .. } => {
                self.execute_devenv_sourcing_hook(exec, devenv).await
            }
        }
    }

    /// Execute basic exec hook for sourcing
    async fn execute_exec_sourcing_hook(
        &self,
        exec: &crate::cue_parser::ExecConfig,
    ) -> Result<HashMap<String, String>> {
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};

        // Create a basic environment for hook execution
        let hook_env = EnvironmentVariables::from_map(
            self.original_env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );

        // Execute the hook command and capture output
        let args = CommandArguments::from_vec(exec.args.clone().unwrap_or_default());
        let executor = crate::command_executor::SystemCommandExecutor::new();

        log::debug!("Executing sourcing hook: {} {:?}", exec.command, exec.args);

        let output = executor
            .execute_with_env(&exec.command, &args, hook_env)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                &exec.command,
                exec.args.clone().unwrap_or_default(),
                format!(
                    "Hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        // Parse the stdout as shell export statements
        let stdout = String::from_utf8_lossy(&output.stdout);
        let env_vars = parse_shell_exports(&stdout);

        log::debug!(
            "Parsed {} environment variables from hook output",
            env_vars.len()
        );

        Ok(env_vars)
    }

    /// Execute a non-sourcing hook (for regular hook execution)
    async fn execute_non_sourcing_hook(&self, hook: &Hook) -> Result<()> {
        match hook {
            Hook::Legacy(hook_config) => {
                // Convert HookConfig to ExecConfig for legacy support
                let exec_config = ExecConfig {
                    command: hook_config.command.clone(),
                    args: Some(hook_config.args.clone()),
                    dir: None,
                    inputs: None,
                    source: hook_config.source,
                    constraints: hook_config.constraints.clone(),
                };
                self.execute_exec_hook(&exec_config).await
            }
            Hook::Exec { exec, .. } => self.execute_exec_hook(exec).await,
            Hook::NixFlake { exec, flake, .. } => self.execute_nix_flake_hook(exec, flake).await,
            Hook::Devenv { exec, devenv, .. } => self.execute_devenv_hook(exec, devenv).await,
        }
    }

    /// Execute basic exec hook (non-sourcing)
    async fn execute_exec_hook(&self, exec: &crate::cue_parser::ExecConfig) -> Result<()> {
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
        log::info!("Executing {} onEnter hooks", on_enter_hooks.len());
=======
        tracing::info!(
            count = %on_enter_hooks.len(),
            "Executing onEnter hooks"
        );
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)

        let env_vars = self.collect_cue_env_vars()?;
        let hook_env = EnvironmentVariables::from_map(env_vars);
        let args = CommandArguments::from_vec(exec.args.clone().unwrap_or_default());
        let executor = crate::command_executor::SystemCommandExecutor::new();

        log::debug!("Executing exec hook: {} {:?}", exec.command, exec.args);

        let output = executor
            .execute_with_env(&exec.command, &args, hook_env)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                &exec.command,
                exec.args.clone().unwrap_or_default(),
                format!(
                    "Hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        // Print hook output if any
        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

<<<<<<< HEAD
        Ok(())
    }

    /// Execute nix flake hook (non-sourcing)
    async fn execute_nix_flake_hook(
        &self,
        _exec: &crate::cue_parser::ExecConfig,
        flake: &crate::cue_parser::NixFlakeConfig,
    ) -> Result<()> {
        // For non-sourcing nix flake hooks, we just execute nix develop
        // instead of print-dev-env (since we're not capturing environment)
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};

        let env_vars = self.collect_cue_env_vars()?;
        let hook_env = EnvironmentVariables::from_map(env_vars);
        let executor = crate::command_executor::SystemCommandExecutor::new();

        // Build nix develop command
        let mut nix_args = vec!["develop".to_string()];

        if let Some(ref reference) = flake.reference {
            nix_args.push(reference.clone());
        } else {
            let flake_dir = flake.dir.as_deref().unwrap_or(".");
            nix_args.push(format!("{flake_dir}#devShell"));
        }

        if let Some(ref shell) = flake.shell {
            nix_args.push(format!("#{shell}"));
        }

        if flake.impure.unwrap_or(false) {
            nix_args.push("--impure".to_string());
        }

        // Add a command to run in the nix develop shell
        nix_args.extend(["--command".to_string(), "true".to_string()]);

        let args = CommandArguments::from_vec(nix_args.clone());

        log::debug!("Executing nix flake hook: nix {nix_args:?}");

        let output = executor.execute_with_env("nix", &args, hook_env).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                "nix",
                nix_args,
                format!(
                    "Nix flake hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Execute devenv hook (non-sourcing)
    async fn execute_devenv_hook(
        &self,
        _exec: &crate::cue_parser::ExecConfig,
        devenv: &crate::cue_parser::DevenvConfig,
    ) -> Result<()> {
        // For non-sourcing devenv hooks, we just execute devenv shell
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};

        let env_vars = self.collect_cue_env_vars()?;
        let hook_env = EnvironmentVariables::from_map(env_vars);
        let executor = crate::command_executor::SystemCommandExecutor::new();

        let mut devenv_args = vec!["shell".to_string()];

        if let Some(ref options) = devenv.options {
            devenv_args.extend(options.clone());
        }

        // Add a command to run in the devenv shell
        devenv_args.extend(["--command".to_string(), "true".to_string()]);

        let args = CommandArguments::from_vec(devenv_args.clone());

        log::debug!("Executing devenv hook: devenv {devenv_args:?}");

        let output = executor.execute_with_env("devenv", &args, hook_env).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                "devenv",
                devenv_args,
                format!(
                    "Devenv hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Execute nix flake hook for sourcing
    async fn execute_nix_flake_sourcing_hook(
        &self,
        _exec: &crate::cue_parser::ExecConfig,
        flake: &crate::cue_parser::NixFlakeConfig,
    ) -> Result<HashMap<String, String>> {
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};

        // Create environment for nix command execution
        let hook_env = EnvironmentVariables::from_map(
            self.original_env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );

        let executor = crate::command_executor::SystemCommandExecutor::new();

        // Build nix print-dev-env command
        let mut nix_args = vec!["print-dev-env".to_string()];

        // Add flake reference if specified, otherwise use current directory
        if let Some(ref reference) = flake.reference {
            nix_args.push(reference.clone());
        } else {
            let flake_dir = flake.dir.as_deref().unwrap_or(".");
            nix_args.push(format!("{flake_dir}#devShell"));
        }

        // Add shell name if specified
        if let Some(ref shell) = flake.shell {
            nix_args.push(format!("#{shell}"));
        }

        // Add impure flag if requested
        if flake.impure.unwrap_or(false) {
            nix_args.push("--impure".to_string());
        }

        let args = CommandArguments::from_vec(nix_args.clone());

        log::debug!("Executing nix flake sourcing hook: nix {nix_args:?}");

        let output = executor.execute_with_env("nix", &args, hook_env).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                "nix",
                nix_args,
                format!(
                    "Nix flake hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        // Parse the stdout as shell export statements
        let stdout = String::from_utf8_lossy(&output.stdout);
        let env_vars = parse_shell_exports(&stdout);

        log::debug!(
            "Parsed {} environment variables from nix flake hook output",
            env_vars.len()
        );

        Ok(env_vars)
    }

    /// Execute devenv hook for sourcing
    async fn execute_devenv_sourcing_hook(
        &self,
        _exec: &crate::cue_parser::ExecConfig,
        devenv: &crate::cue_parser::DevenvConfig,
    ) -> Result<HashMap<String, String>> {
        use crate::command_executor::CommandExecutor;
        use crate::types::{CommandArguments, EnvironmentVariables};

        // Create environment for devenv command execution
        let hook_env = EnvironmentVariables::from_map(
            self.original_env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );

        let executor = crate::command_executor::SystemCommandExecutor::new();

        // Build devenv print-dev-env command
        let mut devenv_args = vec!["print-dev-env".to_string()];

        // Add additional options if specified
        if let Some(ref options) = devenv.options {
            devenv_args.extend(options.clone());
        }

        let args = CommandArguments::from_vec(devenv_args.clone());

        log::debug!("Executing devenv sourcing hook: devenv {devenv_args:?}");

        let output = executor.execute_with_env("devenv", &args, hook_env).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::command_execution(
                "devenv",
                devenv_args,
                format!(
                    "Devenv hook execution failed with status {}: {}",
                    output.status, stderr
                ),
                Some(output.status.code().unwrap_or(-1)),
            ));
        }

        // Parse the stdout as shell export statements
        let stdout = String::from_utf8_lossy(&output.stdout);
        let env_vars = parse_shell_exports(&stdout);

        log::debug!(
            "Parsed {} environment variables from devenv hook output",
            env_vars.len()
        );

        Ok(env_vars)
    }

    /// Apply merged environment variables (sourced + CUE)
    async fn apply_merged_environment(
        &mut self,
        dir: &Path,
        variables: HashMap<String, String>,
        options: &ParseOptions,
    ) -> Result<()> {
        // Build the new environment
        let mut new_env = self.original_env.clone();
        self.cue_vars.clear();

        for (key, value) in variables {
            let expanded_value = match shellexpand::full(&value) {
                Ok(expanded) => expanded.to_string(),
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
        for (name, config) in on_enter_hooks {
            log::debug!("Executing onEnter hook: {name}");
            match crate::async_runtime::run_async(async {
                hook_manager
                    .execute_hook(config, &env_vars)
                    .await
                    .map_err(Error::from)
            }) {
                Ok(_) => log::info!("Successfully executed onEnter hook: {name}"),
=======
        for (name, config) in on_enter_hooks {
            tracing::debug!(
                hook_name = %name,
                "Executing onEnter hook"
            );
            match crate::async_runtime::run_async(async {
                hook_manager
                    .execute_hook(config, &env_vars)
                    .await
                    .map_err(Error::from)
            }) {
                Ok(_) => tracing::info!(
                    hook_name = %name,
                    "Successfully executed onEnter hook"
                ),
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                Err(e) => {
<<<<<<< HEAD
                    return Err(Error::shell_expansion(
                        &value,
                        format!("Failed to expand value for {key}: {e}"),
                    ));
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                    // Log error but continue with other hooks
                    log::error!("Failed to execute onEnter hook '{name}': {e}");
=======
                    // Log error but continue with other hooks
                    tracing::error!(
                        hook_name = %name,
                        error = %e,
                        "Failed to execute onEnter hook"
                    );
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                }
            };

            log::debug!("Setting {key}={expanded_value}");
            new_env.insert(key.clone(), expanded_value.clone());
            self.cue_vars.insert(key.clone(), expanded_value.clone());
            SyncEnv::set_var(key, expanded_value).map_err(|e| Error::Configuration {
                message: format!("Failed to set environment variable: {e}"),
            })?;
        }

        // Create environment diff
        let diff = EnvDiff::new(self.original_env.clone(), new_env);

        // Create file watches
        let mut watches = FileTimes::new();
        let env_cue = dir.join("env.cue");
        if env_cue.exists() {
            watches.watch(&env_cue);
        }

        // Save state
        StateManager::load(
            dir,
            &env_cue,
            options.environment.as_deref(),
            &options.capabilities,
            &diff,
            &watches,
        )
        .await?;

        Ok(())
    }

    fn collect_cue_env_vars(&self) -> Result<HashMap<String, String>> {
        let current_env: HashMap<String, String> = SyncEnv::vars()
            .map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?
            .into_iter()
            .collect();
        let mut cue_env_vars = HashMap::with_capacity(self.cue_vars.len());

        // Collect variables that were added or modified by CUE
        for (key, value) in &current_env {
            if !self.original_env.contains_key(key as &str)
                || self.original_env.get(key as &str) != Some(value)
            {
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

        Ok(cue_env_vars)
    }
}

/// Parse shell export statements from command output
/// Handles formats like:
/// - export VAR=value
/// - VAR=value
/// - export VAR="quoted value"
/// - export VAR='single quoted'
fn parse_shell_exports(output: &str) -> HashMap<String, String> {
    let mut env_vars = HashMap::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Handle export statements: export VAR=value or VAR=value
        let export_line = if let Some(stripped) = line.strip_prefix("export ") {
            stripped
        } else {
            line
        };

        // Find the first = to split key=value
        if let Some(eq_pos) = export_line.find('=') {
            let key = export_line[..eq_pos].trim();
            let value = export_line[eq_pos + 1..].trim();

            // Skip invalid variable names
            if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }

            // Handle quoted values
            let cleaned_value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                // Remove surrounding quotes
                &value[1..value.len() - 1]
            } else {
                value
            };

            env_vars.insert(key.to_string(), cleaned_value.to_string());
        }
    }

    env_vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_and_unload_env() {
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

        let original_value = SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap_or_default();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).await.unwrap();

        assert_eq!(
            SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap(),
            Some("test_value".to_string())
        );

        manager.unload_env().unwrap();

        match original_value {
            Some(val) => assert_eq!(SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap(), Some(val)),
            None => assert!(SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap().is_none()),
        }
    }

    #[tokio::test]
    async fn test_run_command_hermetic() {
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

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).await.unwrap();

        // Set a variable AFTER loading env, so it's not in original_env
        SyncEnv::set_var("TEST_PARENT_VAR", "should_not_exist").unwrap();

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
        let _ = SyncEnv::remove_var("TEST_PARENT_VAR");
    }

    #[tokio::test]
    async fn test_run_command_with_secret_refs() {
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
        manager.load_env(temp_dir.path()).await.unwrap();

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

    #[tokio::test]
    async fn test_run_command_preserves_path_and_home() {
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
        manager.load_env(temp_dir.path()).await.unwrap();

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

    #[tokio::test]
    async fn test_run_command_with_restrictions() {
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
        manager.load_env(temp_dir.path()).await.unwrap();

        // Test without restrictions (should work)
        let restrictions = AccessRestrictions::default();
        let status =
            manager.run_command_with_restrictions("echo", &["test".to_string()], &restrictions);

        // This should work since no restrictions are applied
        assert!(
            status.is_ok(),
            "Command should succeed without restrictions"
        );

        // Test with restrictions (may fail in test environment, but should not panic)
        let restrictions = AccessRestrictions::new(true, true);
        let result =
            manager.run_command_with_restrictions("echo", &["test".to_string()], &restrictions);

        // The result may be Ok or Err depending on environment capabilities
        // What matters is that it doesn't panic and properly handles restrictions
        match result {
            Ok(_) => {
                // Command succeeded (unlikely in restricted environment)
            }
            Err(e) => {
                // Command failed due to restrictions (expected in most test environments)
                let error_msg = e.to_string();
                // Verify the error is related to restrictions/command execution
                assert!(
                    error_msg.contains("CommandExecution")
                        || error_msg.contains("Failed to capture stdout")
                        || error_msg.contains("Failed to spawn command")
                        || error_msg.contains("Network restrictions with Landlock")
                        || error_msg.contains("configuration error"),
                    "Error should be related to command execution with restrictions: {error_msg}"
                );
            }
        }
    }

    #[test]
    fn test_access_restrictions_creation_and_methods() {
        use crate::access_restrictions::AccessRestrictions;

        // Test default (no restrictions)
        let restrictions = AccessRestrictions::default();
        assert!(!restrictions.has_any_restrictions());

        // Test with all restrictions
        let restrictions = AccessRestrictions::new(true, true);
        assert!(restrictions.has_any_restrictions());

        // Test with partial restrictions
        let restrictions = AccessRestrictions::new(true, false);
        assert!(restrictions.has_any_restrictions());

        let restrictions = AccessRestrictions::new(false, true);
        assert!(restrictions.has_any_restrictions());

        let restrictions = AccessRestrictions::new(false, false);
        assert!(!restrictions.has_any_restrictions());
    }

    #[test]
    fn test_parse_shell_exports() {
        // Test basic export statements
        let output = r#"
export PATH=/usr/bin:/bin
export HOME=/home/user
DB_URL=postgres://localhost/test
export API_KEY="secret-key"
export TOKEN='bearer-token'
# This is a comment
export EMPTY_VAR=
INVALID_VAR
export =invalid
export 123INVALID=value
        "#;

        let vars = parse_shell_exports(output);

        assert_eq!(vars.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
        assert_eq!(vars.get("HOME"), Some(&"/home/user".to_string()));
        assert_eq!(
            vars.get("DB_URL"),
            Some(&"postgres://localhost/test".to_string())
        );
        assert_eq!(vars.get("API_KEY"), Some(&"secret-key".to_string()));
        assert_eq!(vars.get("TOKEN"), Some(&"bearer-token".to_string()));
        assert_eq!(vars.get("EMPTY_VAR"), Some(&"".to_string()));

        // Invalid variables should not be included
        assert!(!vars.contains_key("INVALID_VAR"));
        assert!(!vars.contains_key(""));
        assert!(!vars.contains_key("123INVALID"));
    }
}
