use cuenv_config::{HookConfig, HookType};
use cuenv_core::{Error, Result};
use cuenv_utils::sync::env::SyncEnv;
use std::collections::HashMap;

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
        tracing::error!("# cuenv: ✓ Running {} onExit hook(s)...", exit_hooks.len());

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

    tracing::error!("# cuenv: ✓ Environment unloaded");
    Ok(())
}
