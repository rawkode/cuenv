use super::RuntimeExecutor;
use crate::cue_parser::BuildkitRuntimeConfig;
use crate::errors::{Error, Result};
use async_trait::async_trait;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// BuildKit runtime executor - executes commands using BuildKit's Low-Level Builder API
pub struct BuildkitRuntime {
    config: BuildkitRuntimeConfig,
}

impl BuildkitRuntime {
    pub fn new(config: BuildkitRuntimeConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl RuntimeExecutor for BuildkitRuntime {
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

        // Create a temporary Dockerfile if not provided
        let dockerfile_content = if let Some(dockerfile) = &self.config.dockerfile {
            dockerfile.clone()
        } else {
            format!(
                r#"FROM {}
WORKDIR /workspace
COPY . .
RUN {} -c "{}"
"#,
                self.config.image, inner_shell, script_content
            )
        };

        // Write Dockerfile to temporary location
        let temp_dockerfile = working_dir.join(".cuenv-dockerfile");
        fs::write(&temp_dockerfile, dockerfile_content).map_err(|e| {
            Error::file_system(
                temp_dockerfile.clone(),
                "write temporary dockerfile",
                e,
            )
        })?;

        // Build docker build command with BuildKit
        let mut docker_cmd = Command::new("docker");
        docker_cmd.arg("build");

        // Set build context
        let context_path = self.config.context.as_deref().unwrap_or(".");
        docker_cmd.arg(context_path);

        // Use the temporary Dockerfile
        docker_cmd.arg("-f").arg(&temp_dockerfile);

        // Add build arguments
        if let Some(build_args) = &self.config.build_args {
            for (key, value) in build_args {
                docker_cmd.arg("--build-arg").arg(format!("{}={}", key, value));
            }
        }

        // Add target if specified
        if let Some(target) = &self.config.target {
            docker_cmd.arg("--target").arg(target);
        }

        // Add additional buildctl args
        if let Some(extra_args) = &self.config.args {
            for arg in extra_args {
                docker_cmd.arg(arg);
            }
        }

        // Set BuildKit backend
        docker_cmd.env("DOCKER_BUILDKIT", "1");

        // Add a temporary tag
        docker_cmd.arg("-t").arg("cuenv-buildkit-temp");

        // Execute build
        docker_cmd
            .current_dir(working_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let output = docker_cmd.output().map_err(|e| {
            Error::command_execution(
                "docker",
                vec!["build".to_string(), context_path.to_string()],
                format!("Failed to execute task using BuildKit: {e}"),
                None,
            )
        })?;

        // Clean up temporary Dockerfile
        let _ = fs::remove_file(&temp_dockerfile);

        // If using a custom dockerfile with RUN commands, the exit code is from the build
        // For inline execution, we need to run the built image
        if self.config.dockerfile.is_none() {
            Ok(output.status.code().unwrap_or(1))
        } else {
            // Run the built image to execute the command
            let mut run_cmd = Command::new("docker");
            run_cmd.arg("run")
                .arg("--rm")
                .arg("-i");

            // Add environment variables from cuenv
            for (key, value) in env_vars {
                run_cmd.arg("-e").arg(format!("{}={}", key, value));
            }

            run_cmd.arg("cuenv-buildkit-temp")
                .arg(&inner_shell)
                .arg("-c")
                .arg(&script_content)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            let run_output = run_cmd.output().map_err(|e| {
                Error::command_execution(
                    "docker",
                    vec!["run".to_string(), "cuenv-buildkit-temp".to_string()],
                    format!("Failed to run BuildKit image: {e}"),
                    None,
                )
            })?;

            // Clean up the temporary image
            let _ = Command::new("docker")
                .arg("rmi")
                .arg("cuenv-buildkit-temp")
                .output();

            Ok(run_output.status.code().unwrap_or(1))
        }
    }

    fn is_available(&self) -> bool {
        // Check if docker is available and BuildKit is supported
        if let Ok(output) = Command::new("docker")
            .arg("version")
            .arg("--format")
            .arg("{{.Server.Version}}")
            .output()
        {
            // BuildKit is available in Docker 18.06+
            if let Ok(version_str) = String::from_utf8(output.stdout) {
                return !version_str.trim().is_empty();
            }
        }
        false
    }

    fn name(&self) -> &'static str {
        "buildkit"
    }
}