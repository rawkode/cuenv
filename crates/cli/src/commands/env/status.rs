use cuenv_core::Result;
use cuenv_env::EnvManager;
use cuenv_utils::hooks_status::{
    calculate_elapsed, should_show_completed_status, HookState, HooksStatusManager,
};
use std::env;

pub async fn execute(hooks: bool, format: String, verbose: bool) -> Result<()> {
    // Get status for current directory (directory-aware)
    let current_dir = env::current_dir().map_err(|e| {
        cuenv_core::Error::file_system(std::path::PathBuf::from("."), "get current directory", e)
    })?;

    // Try directory-specific status first, then fall back to legacy
    let status = HooksStatusManager::read_status_for_directory(&current_dir)
        .ok()
        .flatten()
        .or_else(|| HooksStatusManager::read_status_from_file().ok());

    // Format output based on requested format
    match format.as_str() {
        "starship" => {
            if let Some(status) = status {
                format_starship_output(&status, verbose);
            }
            // Empty output for Starship when no hooks
        }
        "json" => {
            if let Some(status) = status {
                format_json_output(&status);
            } else {
                // Output empty status for consistency
                tracing::info!("{{}}");
            }
        }
        _ => {
            // Human-readable format: show both hooks and environment
            if let Some(status) = status {
                // Show which directory if available
                if let Some(ref dir) = status.directory {
                    tracing::info!("Directory: {}", dir);
                    tracing::info!("");
                }
                format_human_output(&status);
                tracing::info!(""); // Add spacing
            }

            // Also show environment diff unless hooks flag is set
            if !hooks {
                tracing::info!("Environment Status");
                tracing::info!("==================");
                let env_manager = EnvManager::new();
                env_manager.print_env_diff()?;
            }
        }
    }

    Ok(())
}

fn format_starship_output(status: &cuenv_utils::hooks_status::HooksStatus, verbose: bool) {
    let running_count = status
        .hooks
        .values()
        .filter(|h| h.status == HookState::Running)
        .count();

    let elapsed = calculate_elapsed(status.start_time);

    // Check if all hooks are completed
    if running_count == 0 && status.total > 0 {
        // Check if we should still show completed status (within 5 seconds)
        if should_show_completed_status(status.last_update) {
            if status.failed > 0 {
                // Show failed status
                tracing::info!(
                    "⚠️ {} hook{} failed",
                    status.failed,
                    if status.failed == 1 { "" } else { "s" }
                );
            } else {
                // Show success status
                tracing::info!("✅ Hooks ready");
            }
        }
        // Otherwise show nothing (empty output)
        return;
    }

    // Hooks are still running
    if running_count > 0 {
        if verbose {
            // Show details of currently running hook
            if let Some(running_hook) = status
                .hooks
                .values()
                .find(|h| h.status == HookState::Running)
            {
                let hook_elapsed = calculate_elapsed(running_hook.start_time);
                tracing::info!(
                    "🔄 {} ({}s)",
                    extract_hook_name(&running_hook.name),
                    hook_elapsed.as_secs()
                );
            }
        } else {
            // Show aggregate progress
            let completed = status.completed + status.failed;
            tracing::info!("⏳ {}/{} hooks", completed, status.total);

            // Add duration if hooks have been running for more than 1 second
            if elapsed.as_secs() > 0 {
                tracing::info!(" ({}s)", elapsed.as_secs());
            }
        }
    }
}

fn format_json_output(status: &cuenv_utils::hooks_status::HooksStatus) {
    // Output raw JSON for machine consumption
    if let Ok(json) = serde_json::to_string_pretty(status) {
        tracing::info!("{}", json);
    }
}

fn format_human_output(status: &cuenv_utils::hooks_status::HooksStatus) {
    tracing::info!("Hook Execution Status");
    tracing::info!("=====================");
    tracing::info!("Total hooks: {}", status.total);
    tracing::info!("Completed: {}", status.completed);
    tracing::info!("Failed: {}", status.failed);

    let running_count = status
        .hooks
        .values()
        .filter(|h| h.status == HookState::Running)
        .count();
    tracing::info!("Running: {}", running_count);

    if running_count > 0 {
        tracing::info!("\nCurrently Running:");
        for hook in status.hooks.values() {
            if hook.status == HookState::Running {
                let elapsed = calculate_elapsed(hook.start_time);
                tracing::info!(
                    "  - {} ({}s)",
                    extract_hook_name(&hook.name),
                    elapsed.as_secs()
                );
            }
        }
    }

    if status.failed > 0 {
        tracing::info!("\nFailed Hooks:");
        for hook in status.hooks.values() {
            if hook.status == HookState::Failed {
                tracing::info!("  - {}", extract_hook_name(&hook.name));
                if let Some(error) = &hook.error {
                    tracing::info!("    Error: {}", error);
                }
            }
        }
    }

    let elapsed = calculate_elapsed(status.start_time);
    tracing::info!("\nTotal elapsed time: {}s", elapsed.as_secs());
}

/// Extract a cleaner hook name from the formatted name
fn extract_hook_name(name: &str) -> &str {
    // Hook names are formatted as "HookType:command"
    // Extract just the command part for cleaner display
    name.split(':').next_back().unwrap_or(name)
}
