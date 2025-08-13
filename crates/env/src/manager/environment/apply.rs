use cuenv_config::ParseOptions;
use cuenv_core::{Error, Result};
use cuenv_utils::sync::env::SyncEnv;
use cuenv_utils::FileTimes;
use std::collections::HashMap;
use std::path::Path;

use crate::diff::EnvDiff;
use crate::state::StateManager;

/// Apply merged environment variables (sourced + CUE)
pub async fn apply_merged_environment(
    dir: &Path,
    variables: HashMap<String, String>,
    _options: &ParseOptions,
    has_sourced_env: bool,
    original_env: &HashMap<String, String>,
    cue_vars: &mut HashMap<String, String>,
) -> Result<()> {
    // Build the new environment
    let mut new_env = original_env.clone();
    cue_vars.clear();

    for (key, value) in variables {
        // Skip shell expansion for nix-sourced variables that contain unexpandable references
        // These will be expanded by the shell when the command runs
        let final_value = if has_sourced_env && value.contains("$NIX_BUILD_TOP") {
            // Don't expand nix-specific variables, they'll be set by the shell
            value.clone()
        } else {
            // Try to expand other variables
            match shellexpand::full(&value) {
                Ok(expanded) => expanded.to_string(),
                Err(e) => {
                    // If expansion fails and it's a nix variable, just use it as-is
                    if has_sourced_env && value.contains('$') {
                        tracing::debug!(
                            "Skipping expansion for {key}={value} (will be expanded at runtime)"
                        );
                        value.clone()
                    } else {
                        return Err(Error::shell_expansion(
                            &value,
                            format!("Failed to expand value for {key}: {e}"),
                        ));
                    }
                }
            }
        };

        tracing::debug!("Setting {key}={final_value}");
        new_env.insert(key.clone(), final_value.clone());
        cue_vars.insert(key.clone(), final_value.clone());
        SyncEnv::set_var(key, final_value).map_err(|e| Error::Configuration {
            message: format!("Failed to set environment variable: {e}"),
        })?;
    }

    // Create environment diff
    let diff = EnvDiff::new(original_env.clone(), new_env);

    // Create file watches
    let mut watches = FileTimes::new();
    let env_cue = dir.join("env.cue");
    if env_cue.exists() {
        watches.watch(&env_cue);
    }

    // Save state with all required parameters
    let environment = SyncEnv::var("CUENV_ENV")
        .map_err(|e| Error::Configuration {
            message: format!("Failed to get CUENV_ENV: {e}"),
        })?
        .or_else(|| Some("default".to_string()));

    let capabilities = Vec::new(); // TODO: get actual capabilities from context

    StateManager::load(
        dir,
        &env_cue,
        environment.as_deref(),
        &capabilities,
        &diff,
        &watches,
    )
    .await?;

    Ok(())
}
