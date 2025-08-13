//! Hook execution functionality

use cuenv_config::Hook;
use cuenv_core::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Execute a hook with timeout and capture environment if needed
/// Returns the output and the actual process PID
pub async fn execute_hook_with_timeout(
    hook: &Hook,
    timeout_duration: Duration,
) -> Result<(Option<HashMap<String, String>>, Option<u32>)> {
    // For source hooks, we need to evaluate the output as shell script
    if hook.source.unwrap_or(false) {
        execute_source_hook(hook, timeout_duration).await
    } else {
        execute_regular_hook(hook, timeout_duration).await
    }
}

async fn execute_source_hook(
    hook: &Hook,
    timeout_duration: Duration,
) -> Result<(Option<HashMap<String, String>>, Option<u32>)> {
    // Create a wrapper script that evaluates the hook output and captures env changes
    let wrapper_script = format!(
        r#"
# Save current environment to a temp file with null separation
TEMP_BEFORE=$(mktemp)
TEMP_AFTER=$(mktemp)
trap "rm -f $TEMP_BEFORE $TEMP_AFTER" EXIT

# Save environment with null bytes as separators (handles newlines in values)
env -0 | sort -z > "$TEMP_BEFORE"

# Run the hook command and capture its output
HOOK_OUTPUT="$({} {})"

# Evaluate the output as shell script (this is what direnv does)
eval "$HOOK_OUTPUT"

# Get the new environment with null separation
env -0 | sort -z > "$TEMP_AFTER"

# Output only new/changed variables with null separation for proper parsing
comm -z -13 "$TEMP_BEFORE" "$TEMP_AFTER"
"#,
        hook.command,
        hook.args
            .as_ref()
            .map(|args| args
                .iter()
                .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                .collect::<Vec<_>>()
                .join(" "))
            .unwrap_or_default()
    );

    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(&wrapper_script);

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // No environment variables in Hook struct

    let child = cmd.spawn().map_err(|e| {
        cuenv_core::Error::configuration(format!("Failed to spawn hook process: {e}"))
    })?;

    let pid = child.id();

    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                eprintln!(
                    "# cuenv: Hook {} failed with status: {}",
                    hook.command,
                    output.status
                );
                if !output.stderr.is_empty() {
                    eprintln!(
                        "# cuenv: stderr: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                return Ok((None, pid));
            }

            // Parse the environment changes from null-separated output
            let output_str = String::from_utf8_lossy(&output.stdout);
            let mut env_changes = HashMap::new();

            for entry in output_str.split('\0') {
                if entry.is_empty() {
                    continue;
                }
                if let Some(eq_pos) = entry.find('=') {
                    let key = entry[..eq_pos].to_string();
                    let value = entry[eq_pos + 1..].to_string();
                    env_changes.insert(key, value);
                }
            }

            Ok((Some(env_changes), pid))
        }
        Ok(Err(e)) => Err(cuenv_core::Error::configuration(format!(
            "Failed to execute hook: {e}"
        ))),
        Err(_) => {
            eprintln!("# cuenv: Hook {} timed out", hook.command);
            Ok((None, pid))
        }
    }
}

async fn execute_regular_hook(
    hook: &Hook,
    timeout_duration: Duration,
) -> Result<(Option<HashMap<String, String>>, Option<u32>)> {
    let mut cmd = Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // No environment variables in Hook struct

    let child = cmd.spawn().map_err(|e| {
        cuenv_core::Error::configuration(format!("Failed to spawn hook process: {e}"))
    })?;

    let pid = child.id();

    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                eprintln!(
                    "# cuenv: Hook {} failed with status: {}",
                    hook.command,
                    output.status
                );
                if !output.stderr.is_empty() {
                    eprintln!(
                        "# cuenv: stderr: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Ok((None, pid))
        }
        Ok(Err(e)) => Err(cuenv_core::Error::configuration(format!(
            "Failed to execute hook: {e}"
        ))),
        Err(_) => {
            eprintln!("# cuenv: Hook {} timed out", hook.command);
            Ok((None, pid))
        }
    }
}