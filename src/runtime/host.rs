use super::RuntimeExecutor;
use crate::errors::{Error, Result};
use async_trait::async_trait;
use std::path::Path;
use std::process::{Command, Stdio};

/// Host runtime executor - executes commands directly on the host system
pub struct HostRuntime;

impl HostRuntime {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RuntimeExecutor for HostRuntime {
    async fn execute(
        &self,
        command: Option<&str>,
        script: Option<&str>,
        shell: Option<&str>,
        working_dir: &Path,
        env_vars: &std::collections::HashMap<String, String>,
        args: &[String],
    ) -> Result<i32> {
        // Determine what to execute
        let (shell_cmd, script_content) = match (command, script) {
            (Some(cmd), None) => {
                // Add user args to the command
                let full_command = if args.is_empty() {
                    cmd.to_string()
                } else {
                    format!("{} {}", cmd, args.join(" "))
                };
                (
                    shell.unwrap_or("sh").to_string(),
                    full_command,
                )
            }
            (None, Some(script_text)) => (
                shell.unwrap_or("sh").to_string(),
                script_text.to_string(),
            ),
            (Some(_), Some(_)) => {
                return Err(Error::configuration(
                    "Task cannot have both 'command' and 'script' defined".to_string(),
                ));
            }
            (None, None) => {
                return Err(Error::configuration(
                    "Task must have either 'command' or 'script' defined".to_string(),
                ));
            }
        };

        // Execute the task
        let mut cmd = Command::new(&shell_cmd);
        cmd.arg("-c")
            .arg(&script_content)
            .current_dir(working_dir)
            .env_clear() // Clear environment
            .envs(env_vars) // Set cuenv environment variables
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let output = cmd.output().map_err(|e| {
            Error::command_execution(
                &shell_cmd,
                vec!["-c".to_string(), script_content.clone()],
                format!("Failed to execute task: {e}"),
                None,
            )
        })?;

        Ok(output.status.code().unwrap_or(1))
    }

    fn is_available(&self) -> bool {
        true // Host runtime is always available
    }

    fn name(&self) -> &'static str {
        "host"
    }
}