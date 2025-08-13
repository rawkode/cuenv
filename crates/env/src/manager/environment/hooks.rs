use cuenv_config::{Hook, HookConfig, HookType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::preload::PreloadHookManager;
use super::preload_supervisor;
use crate::manager::hooks;

/// Process all hooks: source hooks synchronously, start preload hooks in background, return preload manager
pub async fn process_hooks_with_preload(
    dir: &Path,
    hook_list: &HashMap<String, Vec<Hook>>,
) -> (HashMap<String, String>, PreloadHookManager) {
    let mut sourced_env_vars = HashMap::new();
    let preload_manager = PreloadHookManager::new();
    let mut preload_hooks = Vec::new();
    let mut regular_hooks = Vec::new();

    // Check if we're in shell hook mode
    let is_shell_hook = std::env::var("CUENV_SHELL_HOOK").is_ok();

    // Categorize hooks
    for (hook_type, hook_vec) in hook_list {
        if hook_type == "onEnter" {
            for hook in hook_vec {
                // In shell hook context: preload hooks run in background
                // In other contexts: all hooks run synchronously
                if hook.preload.unwrap_or(false) && is_shell_hook {
                    // Preload hooks run in background ONLY in shell hook context
                    preload_hooks.push(hook.clone());
                } else if hook.source.unwrap_or(false) {
                    // Source hooks always run synchronously to capture environment
                    // This includes preload+source hooks in non-shell contexts
                    tracing::info!("Processing source hook: {}", hook.command);

                    if let Ok(cache) = crate::cache::EnvCache::new(dir) {
                        match hooks::execute_hook(hook, &cache, false).await {
                            Ok((env_vars, _file_times)) => {
                                tracing::info!(
                                    "Loaded {} variables from {} hook",
                                    env_vars.len(),
                                    hook.command
                                );
                                sourced_env_vars.extend(env_vars);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to execute {} hook: {}", hook.command, e);
                            }
                        }
                    }
                } else {
                    // Regular hooks - execute synchronously
                    regular_hooks.push(hook.clone());
                }
            }
        }
    }

    // Start preload hooks in background using supervisor
    if !preload_hooks.is_empty() {
        tracing::info!(
            "Starting {} preload hooks with supervisor",
            preload_hooks.len()
        );
        
        // Spawn the supervisor as a separate process
        spawn_preload_supervisor(preload_hooks).await;
    }

    // Execute regular hooks synchronously
    for hook in regular_hooks {
        tracing::info!("Executing hook: {} {:?}", hook.command, hook.args);
        if let Err(e) = execute_regular_hook(&hook).await {
            tracing::warn!("Failed to execute hook: {}: {}", hook.command, e);
        }
    }

    (sourced_env_vars, preload_manager)
}

/// Execute a regular (non-source, non-preload) hook synchronously
async fn execute_regular_hook(hook: &Hook) -> cuenv_core::Result<()> {
    use std::process::Stdio;

    let mut cmd = tokio::process::Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // If we're in shell hook mode, redirect stdout to stderr to prevent
    // hook output from interfering with shell export commands
    if std::env::var("CUENV_SHELL_HOOK").is_ok() {
        cmd.stdout(Stdio::from(std::io::stderr()));
    }

    let status = cmd.status().await.map_err(|e| {
        cuenv_core::Error::command_execution(
            hook.command.clone(),
            hook.args.clone().unwrap_or_default(),
            format!("Failed to execute hook: {e}"),
            None,
        )
    })?;

    if !status.success() {
        tracing::warn!("Hook command failed with status: {:?}", status.code());
    }

    Ok(())
}

/// Process sourcing hooks to capture environment variables
#[allow(dead_code)]
pub async fn process_sourcing_hooks(
    dir: &Path,
    hook_list: &HashMap<String, Vec<Hook>>,
) -> HashMap<String, String> {
    let mut sourced_env_vars = HashMap::new();

    // Process onEnter hooks that provide environment
    let cache = crate::cache::EnvCache::new(dir).ok();

    for (hook_type, hook_vec) in hook_list {
        if hook_type == "onEnter" {
            for hook in hook_vec {
                // All hooks are now simple ExecHooks
                if hook.source.unwrap_or(false) {
                    tracing::info!("Processing source hook: {}", hook.command);

                    if let Some(ref cache) = cache {
                        match hooks::execute_hook(hook, cache, false).await {
                            Ok((env_vars, _file_times)) => {
                                tracing::info!(
                                    "Loaded {} variables from {} hook",
                                    env_vars.len(),
                                    hook.command
                                );
                                sourced_env_vars.extend(env_vars);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to execute {} hook: {}", hook.command, e);
                            }
                        }
                    }
                } else {
                    // Non-sourcing hooks will be executed later
                    tracing::debug!("Skipping non-source hook: {}", hook.command);
                }
            }
        }
    }

    sourced_env_vars
}

/// Execute onEnter hooks
pub fn execute_on_enter_hooks(hooks: &HashMap<String, HookConfig>) -> cuenv_core::Result<()> {
    // Filter for onEnter hooks
    let on_enter_hooks: Vec<(&String, &HookConfig)> = hooks
        .iter()
        .filter(|(_, config)| config.hook_type == HookType::OnEnter)
        .collect();

    if on_enter_hooks.is_empty() {
        return Ok(());
    }

    // Hook execution is temporarily disabled to resolve circular dependency
    // See comments in original code for details
    tracing::info!(count = %on_enter_hooks.len(), "Would execute onEnter hooks");

    Ok(())
}

/// Spawn the preload supervisor as a background process
async fn spawn_preload_supervisor(hooks: Vec<Hook>) {
    // Serialize hooks to pass to supervisor process
    let hooks_json = match serde_json::to_string(&hooks) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("Failed to serialize hooks: {}", e);
            return;
        }
    };
    
    // Get the current executable path
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            tracing::error!("Failed to get current executable: {}", e);
            return;
        }
    };
    
    // Spawn supervisor as a detached process
    let result = tokio::process::Command::new(&exe_path)
        .arg("supervisor")
        .arg("--hooks")
        .arg(&hooks_json)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
        
    match result {
        Ok(mut child) => {
            // Detach the child process so it continues running
            match child.id() {
                Some(pid) => {
                    tracing::info!("Spawned preload supervisor with PID: {}", pid);
                    // Don't wait for it - let it run in background
                    std::mem::forget(child);
                }
                None => {
                    tracing::warn!("Failed to get supervisor PID");
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to spawn supervisor: {}", e);
            
            // Fallback to running hooks directly
            tokio::spawn(async move {
                if let Err(e) = preload_supervisor::run_supervisor(hooks).await {
                    tracing::error!("Supervisor failed: {}", e);
                }
            });
        }
    }
}

/// Load captured environment from supervisor
pub fn load_captured_environment() -> Option<HashMap<String, String>> {
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    let latest_file = cache_dir.join("latest_env.json");
    
    if !latest_file.exists() {
        return None;
    }
    
    match std::fs::read_to_string(&latest_file) {
        Ok(content) => {
            match serde_json::from_str::<preload_supervisor::CapturedEnvironment>(&content) {
                Ok(captured) => {
                    tracing::info!("Loaded {} environment variables from supervisor cache", captured.env_vars.len());
                    Some(captured.env_vars)
                }
                Err(e) => {
                    tracing::warn!("Failed to parse captured environment: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read captured environment: {}", e);
            None
        }
    }
}
