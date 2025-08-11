use cuenv_config::{Hook, HookConfig, HookType};
use std::collections::HashMap;
use std::path::Path;

use crate::manager::hooks;

/// Process sourcing hooks to capture environment variables
pub async fn process_sourcing_hooks(
    dir: &Path,
    hook_list: &HashMap<String, Vec<Hook>>,
) -> HashMap<String, String> {
    let mut sourced_env_vars = HashMap::new();

    // Process onEnter hooks that provide environment (nix flake, devenv, source hooks)
    let cache = crate::cache::EnvCache::new(dir).ok();

    for (hook_type, hook_vec) in hook_list {
        if hook_type == "onEnter" {
            for hook in hook_vec {
                match hook {
                    Hook::SimpleNixFlake { flake: _ } | Hook::NixFlake { flake: _, .. } => {
                        if let Some(ref cache) = cache {
                            match hooks::execute_nix_flake_hook("dummy_flake", cache, false).await {
                                Ok((env_vars, _file_times)) => {
                                    tracing::info!(
                                        "Loaded {} variables from nix flake",
                                        env_vars.len()
                                    );
                                    sourced_env_vars.extend(env_vars);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to execute nix flake hook: {}", e);
                                }
                            }
                        }
                    }
                    Hook::SimpleDevenv { devenv: _ } | Hook::Devenv { devenv: _, .. } => {
                        tracing::info!("Processing devenv hook");
                        if let Some(ref cache) = cache {
                            match hooks::execute_devenv_hook("dummy_devenv", cache, false).await {
                                Ok((env_vars, _file_times)) => {
                                    tracing::info!(
                                        "Loaded {} variables from devenv",
                                        env_vars.len()
                                    );
                                    sourced_env_vars.extend(env_vars);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to execute devenv hook: {}", e);
                                }
                            }
                        }
                    }
                    Hook::Exec { exec, .. } if exec.source.unwrap_or(false) => {
                        tracing::info!("Processing source hook: {}", exec.command);
                        match hooks::execute_source_hook("dummy_exec", cache.as_ref()).await {
                            Ok((env_vars, _file_times)) => {
                                tracing::info!(
                                    "Loaded {} variables from source hook",
                                    env_vars.len()
                                );
                                sourced_env_vars.extend(env_vars);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to execute source hook: {}", e);
                            }
                        }
                    }
                    _ => {
                        // Non-sourcing hooks will be executed later
                    }
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
