use super::RuntimeExecutor;
use crate::cue_parser::NixRuntimeConfig;
use crate::errors::{Error, Result};
use async_trait::async_trait;
use std::path::Path;
use std::process::{Command, Stdio};

/// Nix runtime executor - executes commands within nix-shell environments
pub struct NixRuntime {
    config: NixRuntimeConfig,
}

impl NixRuntime {
    pub fn new(config: NixRuntimeConfig) -> Self {
        Self { config }
    }
}

impl Default for NixRuntimeConfig {
    fn default() -> Self {
        Self {
            shell: None,
            flake: None,
            pure: Some(false),
            args: None,
        }
    }
}

#[async_trait]
impl RuntimeExecutor for NixRuntime {
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
        let (inner_shell, script_content) = match (command, script) {
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

        // Build nix command
        let mut nix_cmd = if let Some(flake) = &self.config.flake {
            // Use nix develop for flakes
            let mut cmd = Command::new("nix");
            cmd.arg("develop");
            if flake == "." {
                // Current directory flake
                cmd.arg(".");
            } else {
                cmd.arg(flake);
            }
            cmd
        } else if let Some(shell_expr) = &self.config.shell {
            // Use nix-shell with expression
            let mut cmd = Command::new("nix-shell");
            cmd.arg("-p").arg(shell_expr);
            cmd
        } else {
            // Default nix-shell
            let mut cmd = Command::new("nix-shell");
            cmd.arg("shell.nix");
            cmd
        };

        // Add pure flag if specified
        if self.config.pure.unwrap_or(false) {
            nix_cmd.arg("--pure");
        }

        // Add additional args
        if let Some(extra_args) = &self.config.args {
            for arg in extra_args {
                nix_cmd.arg(arg);
            }
        }

        // Add command to execute
        nix_cmd.arg("--command")
            .arg(&inner_shell)
            .arg("-c")
            .arg(&script_content)
            .current_dir(working_dir)
            .env_clear() // Clear environment
            .envs(env_vars) // Set cuenv environment variables
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let output = nix_cmd.output().map_err(|e| {
            Error::command_execution(
                "nix",
                vec!["develop".to_string(), "--command".to_string(), inner_shell.clone()],
                format!("Failed to execute task in nix environment: {e}"),
                None,
            )
        })?;

        Ok(output.status.code().unwrap_or(1))
    }

    fn is_available(&self) -> bool {
        // Check if nix is available
        Command::new("nix")
            .arg("--version")
            .output()
            .is_ok()
    }

    fn name(&self) -> &'static str {
        "nix"
    }
}