use super::RuntimeExecutor;
use crate::cue_parser::PodmanRuntimeConfig;
use crate::errors::{Error, Result};
use async_trait::async_trait;
use std::path::Path;
use std::process::{Command, Stdio};

/// Podman runtime executor - executes commands within Podman containers
pub struct PodmanRuntime {
    config: PodmanRuntimeConfig,
}

impl PodmanRuntime {
    pub fn new(config: PodmanRuntimeConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl RuntimeExecutor for PodmanRuntime {
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

        // Build podman run command
        let mut podman_cmd = Command::new("podman");
        podman_cmd.arg("run");

        // Add rm flag if specified (default to true)
        if self.config.rm.unwrap_or(true) {
            podman_cmd.arg("--rm");
        }

        // Add interactive and tty flags
        podman_cmd.arg("-i");

        // Set working directory
        let container_workdir = self.config.work_dir.as_deref().unwrap_or("/workspace");
        podman_cmd.arg("-w").arg(container_workdir);

        // Mount current working directory
        let host_workdir = working_dir.to_string_lossy();
        podman_cmd.arg("-v").arg(format!("{}:{}", host_workdir, container_workdir));

        // Add volume mounts
        if let Some(volumes) = &self.config.volumes {
            for volume in volumes {
                podman_cmd.arg("-v").arg(volume);
            }
        }

        // Add environment variables from cuenv
        for (key, value) in env_vars {
            podman_cmd.arg("-e").arg(format!("{}={}", key, value));
        }

        // Add environment variables from config
        if let Some(config_env_vars) = &self.config.env {
            for (key, value) in config_env_vars {
                podman_cmd.arg("-e").arg(format!("{}={}", key, value));
            }
        }

        // Add network configuration
        if let Some(network) = &self.config.network {
            podman_cmd.arg("--network").arg(network);
        }

        // Add additional podman args
        if let Some(extra_args) = &self.config.args {
            for arg in extra_args {
                podman_cmd.arg(arg);
            }
        }

        // Add image
        podman_cmd.arg(&self.config.image);

        // Add command to execute
        podman_cmd.arg(&inner_shell)
            .arg("-c")
            .arg(&script_content)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let output = podman_cmd.output().map_err(|e| {
            Error::command_execution(
                "podman",
                vec!["run".to_string(), self.config.image.clone()],
                format!("Failed to execute task in podman container: {e}"),
                None,
            )
        })?;

        Ok(output.status.code().unwrap_or(1))
    }

    fn is_available(&self) -> bool {
        // Check if podman is available
        Command::new("podman")
            .arg("--version")
            .output()
            .is_ok()
    }

    fn name(&self) -> &'static str {
        "podman"
    }
}