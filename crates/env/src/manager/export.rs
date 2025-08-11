use cuenv_core::{Error, Result};
use cuenv_utils::sync::env::SyncEnv;
use std::collections::HashMap;

use super::stubs::{Platform, Shell};

/// Export environment changes for a specific shell
pub fn export_for_shell(
    original_env: &HashMap<String, String>,
    shell: &str,
) -> Result<String> {
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
        if !original_env.contains_key(key as &str)
            || original_env.get(key as &str) != Some(value)
        {
            output.push_str(&format.format_export(key, value));
            output.push('\n');
        }
    }

    // Unset removed variables
    for key in original_env.keys() {
        if !current_env.contains_key(key) {
            output.push_str(&format.format_unset(key));
            output.push('\n');
        }
    }

    Ok(output)
}

/// Print environment diff to stdout/stderr
pub fn print_env_diff(original_env: &HashMap<String, String>) -> Result<()> {
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
            if let Some(original) = original_env.get(key) {
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

        for (key, value) in original_env {
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
            if let Some(original) = original_env.get(key) {
                if original != value {
                    println!("  {key} (modified): {original} -> {value}");
                }
            } else {
                println!("  {key} (new): {value}");
            }
        }

        for (key, value) in original_env {
            if !current_env.contains_key(key) {
                println!("  {key} (removed): {value}");
            }
        }
    }

    Ok(())
}