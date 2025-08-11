use cuenv_config::{HookConfig, HookType};
use cuenv_core::{Error, Result};
use cuenv_utils::sync::env::SyncEnv;
use std::collections::HashMap;

use crate::manager::stubs::Platform;

/// Unload environment and restore original
pub fn unload_env(
    original_env: &HashMap<String, String>,
    hooks: &HashMap<String, HookConfig>,
    cue_vars: &mut HashMap<String, String>,
    cue_vars_metadata: &mut HashMap<String, cuenv_config::VariableMetadata>,
) -> Result<()> {
    // Execute onExit hooks before unloading environment
    let exit_hooks: Vec<(&String, &HookConfig)> = hooks
        .iter()
        .filter(|(_, config)| config.hook_type == HookType::OnExit)
        .collect();
    if !exit_hooks.is_empty() {
        tracing::info!(count = %exit_hooks.len(), "Executing onExit hooks");

        // Hook execution is temporarily disabled to resolve circular dependency
        // See comments in original code for details
    }

    // Restore original environment
    let current_env: Vec<(String, String)> = SyncEnv::vars().map_err(|e| Error::Configuration {
        message: format!("Failed to get environment variables: {e}"),
    })?;

    for (key, _) in current_env {
        if let Some(original_value) = original_env.get(&key) {
            SyncEnv::set_var(&key, original_value).map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?;
        } else if !original_env.contains_key(&key) {
            SyncEnv::remove_var(&key).map_err(|e| Error::Configuration {
                message: format!("Failed to get environment variables: {e}"),
            })?;
        }
    }

    // Clear CUE vars and metadata
    cue_vars.clear();
    cue_vars_metadata.clear();

    Ok(())
}

/// Collect CUE environment variables for hook execution
#[allow(dead_code)]
pub fn collect_cue_env_vars(
    cue_vars: &HashMap<String, String>,
    original_env: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let current_env: HashMap<String, String> = SyncEnv::vars()
        .map_err(|e| Error::Configuration {
            message: format!("Failed to get environment variables: {e}"),
        })?
        .into_iter()
        .collect();
    let mut cue_env_vars = HashMap::with_capacity(cue_vars.len());

    // Collect variables that were added or modified by CUE
    for (key, value) in &current_env {
        if !original_env.contains_key(key as &str) || original_env.get(key as &str) != Some(value)
        {
            cue_env_vars.insert(key.clone(), value.clone());
        }
    }

    // Add minimal required environment variables for hook execution
    if let Some(path) = original_env.get("PATH") {
        cue_env_vars.insert("PATH".to_string(), path.clone());
    }

    // Ensure HOME directory is available
    let home_var = Platform::home_env_var();
    if let Some(home_value) = original_env.get(home_var) {
        cue_env_vars.insert(home_var.to_string(), home_value.clone());
    }

    // Ensure HOME is set on all platforms for compatibility
    if let Some(home) = original_env.get("HOME") {
        cue_env_vars.insert("HOME".to_string(), home.clone());
    }

    Ok(cue_env_vars)
}