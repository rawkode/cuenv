use cuenv_config::Hook;
use cuenv_core::Result;
use cuenv_utils::FileTimes;
use std::collections::HashMap;
use std::process::{Command, Stdio};

/// Execute a hook and optionally capture its environment
pub async fn execute_hook(
    hook: &Hook,
    cache: &crate::cache::EnvCache,
    force_reload: bool,
) -> Result<(HashMap<String, String>, FileTimes)> {
    // Check cache first unless forced to reload
    if !force_reload && hook.source.unwrap_or(false) {
        if let Ok(cached_env) = cache.load() {
            // Check if cache is still valid based on inputs
            if let Some(inputs) = &hook.inputs {
                let watch_files: Vec<std::path::PathBuf> =
                    inputs.iter().map(std::path::PathBuf::from).collect();
                let watcher = crate::FileWatcher::new(watch_files);

                if watcher.cache_is_valid(&cache.cache_file()) {
                    tracing::info!("Using cached environment for {} hook", hook.command);
                    return Ok((cached_env, FileTimes::new()));
                }
            }
        }
    }

    tracing::info!("Executing hook: {} {:?}", hook.command, hook.args);

    // Build command
    let mut cmd = Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // If this is a source hook, capture output
    if hook.source.unwrap_or(false) {
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let output = cmd.output().map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute hook: {e}"),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Hook command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Save raw output to cache
        cache.save_rc(&stdout).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to save hook output: {e}"))
        })?;

        // Parse environment variables from output
        let env = crate::evaluate_shell_environment(&stdout).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to parse hook output: {e}"))
        })?;

        // Filter and save to cache
        let env = crate::filter_environment(env);
        cache.save(&env).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to save environment: {e}"))
        })?;

        Ok((env, FileTimes::new()))
    } else {
        // Just execute without capturing
        let status = cmd.status().map_err(|e| {
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

        Ok((HashMap::new(), FileTimes::new()))
    }
}
