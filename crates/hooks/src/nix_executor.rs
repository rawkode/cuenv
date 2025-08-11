use cuenv_core::Result;
use std::process::{Command, Stdio};

/// Execute a command and capture its output
pub fn execute_command(mut cmd: Command) -> Result<String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let output = cmd.output().map_err(|e| {
        cuenv_core::Error::configuration(format!("Failed to execute command: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            tracing::warn!("Command stderr: {}", stderr);
        }

        // For nix/devenv, we might still have useful output even on non-zero exit
        // (e.g., warnings that don't prevent environment generation)
        if output.stdout.is_empty() {
            return Err(cuenv_core::Error::configuration(format!(
                "Command failed with status {:?}: {}",
                output.status.code(),
                stderr
            )));
        }
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
