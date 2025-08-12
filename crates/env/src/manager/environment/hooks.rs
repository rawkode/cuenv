use cuenv_config::{Hook, HookConfig, HookType};
use std::collections::HashMap;
use std::path::Path;

use super::preload::PreloadHookManager;
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

    // Categorize hooks
    for (hook_type, hook_vec) in hook_list {
        if hook_type == "onEnter" {
            for hook in hook_vec {
                if hook.source.unwrap_or(false) {
                    // Source hooks - execute synchronously
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
                } else if hook.preload.unwrap_or(false) {
                    // Preload hooks - will be executed in background
                    preload_hooks.push(hook.clone());
                } else {
                    // Regular hooks - will be executed synchronously after environment is set
                    regular_hooks.push(hook.clone());
                }
            }
        }
    }

    // Start preload hooks in background
    if !preload_hooks.is_empty() {
        tracing::info!(
            "Starting {} preload hooks in background",
            preload_hooks.len()
        );
        if let Err(e) = preload_manager.execute_preload_hooks(preload_hooks).await {
            tracing::warn!("Failed to start preload hooks: {}", e);
        }
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
    let mut cmd = tokio::process::Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
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
