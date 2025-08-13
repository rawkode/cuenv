use cuenv_config::{Hook, HookConfig, HookType};
use std::collections::HashMap;
use std::path::Path;

use super::supervisor;
use crate::manager::hooks;
use supervisor::{Supervisor, SupervisorMode};

/// Process all hooks using the new supervisor-based model.
pub async fn process_all_hooks(
    dir: &Path,
    hook_list: &HashMap<String, Vec<Hook>>,
) -> cuenv_core::Result<HashMap<String, String>> {
    let mut on_enter_hooks = Vec::new();

    // Collect all onEnter hooks
    if let Some(hooks) = hook_list.get("onEnter") {
        on_enter_hooks.extend(hooks.clone());
    }

    // If there are no hooks, we're done.
    if on_enter_hooks.is_empty() {
        return Ok(HashMap::new());
    }

    // Run all onEnter hooks through the supervisor in foreground mode with directory context.
    let supervisor =
        Supervisor::new_for_directory(dir, on_enter_hooks, SupervisorMode::Foreground)?;
    supervisor.run().await?;

    // Read the captured environment from the directory-specific cache
    let cache_dir = cuenv_utils::paths::get_state_dir(dir);
    let latest_file = cache_dir.join("latest_env.json");
    if latest_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&latest_file) {
            if let Ok(captured) = serde_json::from_str::<supervisor::CapturedEnvironment>(&content)
            {
                tracing::info!(
                    "Loaded {} environment variables from supervisor cache",
                    captured.env_vars.len()
                );
                return Ok(captured.env_vars);
            }
        }
    }

    Ok(HashMap::new())
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
pub async fn spawn_supervisor(hooks: Vec<Hook>) {
    // In a production build, run the supervisor in background mode
    #[cfg(not(debug_assertions))]
    {
        tokio::spawn(async move {
            match Supervisor::new(hooks, SupervisorMode::Background) {
                Ok(supervisor) => {
                    if let Err(e) = supervisor.run().await {
                        tracing::error!("Supervisor failed: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create supervisor: {}", e);
                }
            }
        });
    }

    // In debug builds, log what would happen
    #[cfg(debug_assertions)]
    {
        tracing::debug!("Would spawn supervisor with {} hooks", hooks.len());
    }
}

/// Load captured environment from supervisor for the current directory
pub fn load_captured_environment() -> Option<HashMap<String, String>> {
    let current_dir = std::env::current_dir().ok()?;
    load_captured_environment_for_directory(&current_dir)
}

/// Load captured environment from supervisor for a specific directory
pub fn load_captured_environment_for_directory(
    directory: &Path,
) -> Option<HashMap<String, String>> {
    let cache_dir = cuenv_utils::paths::get_state_dir(directory);
    let file_path = cache_dir.join("latest_env.json");

    if let Ok(content) = std::fs::read_to_string(&file_path) {
        match serde_json::from_str::<supervisor::CapturedEnvironment>(&content) {
            Ok(captured) => {
                tracing::info!(
                    "Loaded {} environment variables from supervisor cache",
                    captured.env_vars.len()
                );

                // Delete the file after successfully loading to avoid re-sourcing
                let _ = std::fs::remove_file(&file_path);

                Some(captured.env_vars)
            }
            Err(e) => {
                tracing::warn!("Failed to parse captured environment: {}", e);
                None
            }
        }
    } else {
        None
    }
}
