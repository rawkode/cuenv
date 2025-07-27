//! Sandboxed execution environment for hermetic builds
//!
//! This module provides various sandboxing mechanisms to ensure
//! build hermeticity and reproducibility.

use crate::remote_cache::{RemoteCacheError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

/// Sandbox mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// No sandboxing (for testing)
    None,
    /// Basic process isolation
    Basic,
    /// Use Linux namespaces (requires privileges)
    Namespaces,
    /// Use landlock for filesystem isolation
    Landlock,
    /// Use Docker/OCI containers
    Container,
}

impl Default for SandboxMode {
    fn default() -> Self {
        // Use landlock by default on Linux if available
        #[cfg(target_os = "linux")]
        {
            if std::path::Path::new("/sys/kernel/security/landlock").exists() {
                return SandboxMode::Landlock;
            }
        }
        SandboxMode::Basic
    }
}

/// Sandbox configuration
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Sandboxing mode
    pub mode: SandboxMode,
    /// Working directory inside sandbox
    pub working_dir: PathBuf,
    /// Allowed read paths
    pub read_paths: Vec<PathBuf>,
    /// Allowed write paths
    pub write_paths: Vec<PathBuf>,
    /// Allowed execute paths
    pub exec_paths: Vec<PathBuf>,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Network access
    pub allow_network: bool,
    /// Maximum memory (bytes)
    pub memory_limit: Option<u64>,
    /// Maximum CPU time (seconds)
    pub cpu_limit: Option<u64>,
    /// User/group ID for execution
    pub uid_gid: Option<(u32, u32)>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            mode: SandboxMode::default(),
            working_dir: PathBuf::from("/tmp/cuenv-sandbox"),
            read_paths: vec![],
            write_paths: vec![],
            exec_paths: vec![],
            env_vars: HashMap::new(),
            allow_network: false,
            memory_limit: Some(4 * 1024 * 1024 * 1024), // 4GB
            cpu_limit: Some(3600),                      // 1 hour
            uid_gid: None,
        }
    }
}

/// Sandbox for hermetic execution
pub struct Sandbox {
    config: SandboxConfig,
}

impl Sandbox {
    /// Create a new sandbox
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Execute a command in the sandbox
    pub async fn execute(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        match self.config.mode {
            SandboxMode::None => self.execute_none(command, stdin).await,
            SandboxMode::Basic => self.execute_basic(command, stdin).await,
            SandboxMode::Namespaces => self.execute_namespaces(command, stdin).await,
            SandboxMode::Landlock => self.execute_landlock(command, stdin).await,
            SandboxMode::Container => self.execute_container(command, stdin).await,
        }
    }

    /// Execute without sandboxing
    async fn execute_none(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        if command.is_empty() {
            return Err(RemoteCacheError::Sandbox("Empty command".to_string()));
        }

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..])
            .current_dir(&self.config.working_dir)
            .envs(&self.config.env_vars)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let start = std::time::Instant::now();
        let mut child = cmd.spawn()?;

        // Write stdin if provided
        if let Some(input) = stdin {
            if let Some(mut stdin_handle) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin_handle.write_all(&input).await?;
                stdin_handle.flush().await?;
            }
        }

        let output = child.wait_with_output().await?;
        let duration = start.elapsed();

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
            duration,
        })
    }

    /// Execute with basic process isolation
    async fn execute_basic(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        if command.is_empty() {
            return Err(RemoteCacheError::Sandbox("Empty command".to_string()));
        }

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..])
            .current_dir(&self.config.working_dir)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Clear environment and set only allowed vars
        cmd.env_clear();
        cmd.envs(&self.config.env_vars);

        // Set resource limits on Unix
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            let memory_limit = self.config.memory_limit;
            let cpu_limit = self.config.cpu_limit;

            unsafe {
                cmd.pre_exec(move || {
                    // Set memory limit
                    if let Some(limit) = memory_limit {
                        let rlimit = libc::rlimit {
                            rlim_cur: limit as libc::rlim_t,
                            rlim_max: limit as libc::rlim_t,
                        };
                        if libc::setrlimit(libc::RLIMIT_AS, &rlimit) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    }

                    // Set CPU time limit
                    if let Some(limit) = cpu_limit {
                        let rlimit = libc::rlimit {
                            rlim_cur: limit as libc::rlim_t,
                            rlim_max: limit as libc::rlim_t,
                        };
                        if libc::setrlimit(libc::RLIMIT_CPU, &rlimit) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    }

                    Ok(())
                });
            }
        }

        let start = std::time::Instant::now();
        let mut child = cmd.spawn()?;

        // Write stdin if provided
        if let Some(input) = stdin {
            if let Some(mut stdin_handle) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin_handle.write_all(&input).await?;
                stdin_handle.flush().await?;
            }
        }

        let output = child.wait_with_output().await?;
        let duration = start.elapsed();

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
            duration,
        })
    }

    /// Execute with Linux namespaces
    async fn execute_namespaces(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        #[cfg(target_os = "linux")]
        {
            // Use unshare to create new namespaces
            let mut unshare_cmd = Command::new("unshare");
            unshare_cmd.arg("--mount").arg("--pid").arg("--fork");

            if !self.config.allow_network {
                unshare_cmd.arg("--net");
            }

            // Add the actual command
            unshare_cmd.arg("--");
            unshare_cmd.args(&command);

            unshare_cmd
                .current_dir(&self.config.working_dir)
                .env_clear()
                .envs(&self.config.env_vars)
                .stdin(if stdin.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let start = std::time::Instant::now();
            let mut child = unshare_cmd.spawn()?;

            // Write stdin if provided
            if let Some(input) = stdin {
                if let Some(mut stdin_handle) = child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    stdin_handle.write_all(&input).await?;
                    stdin_handle.flush().await?;
                }
            }

            let output = child.wait_with_output().await?;
            let duration = start.elapsed();

            Ok(SandboxResult {
                exit_code: output.status.code().unwrap_or(-1),
                stdout: output.stdout,
                stderr: output.stderr,
                duration,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Fall back to basic sandboxing
            self.execute_basic(command, stdin).await
        }
    }

    /// Execute with landlock sandboxing
    async fn execute_landlock(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        #[cfg(target_os = "linux")]
        {
            use landlock::{Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI};

            // Create landlock ruleset
            let abi = ABI::V3;
            let mut ruleset = Ruleset::default()
                .handle_access(AccessFs::from_all(abi))?
                .create()?;

            // Add read permissions
            for path in &self.config.read_paths {
                ruleset = ruleset.add_rule(landlock::PathBeneath::new(
                    path,
                    AccessFs::ReadFile | AccessFs::ReadDir,
                ))?;
            }

            // Add write permissions
            for path in &self.config.write_paths {
                ruleset = ruleset.add_rule(landlock::PathBeneath::new(
                    path,
                    AccessFs::WriteFile | AccessFs::MakeDir | AccessFs::Remove,
                ))?;
            }

            // Add execute permissions
            for path in &self.config.exec_paths {
                ruleset = ruleset.add_rule(landlock::PathBeneath::new(path, AccessFs::Execute))?;
            }

            // Apply ruleset
            let status = ruleset.restrict_self()?;

            // Now execute with basic sandboxing
            self.execute_basic(command, stdin).await
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Fall back to basic sandboxing
            self.execute_basic(command, stdin).await
        }
    }

    /// Execute in a container
    async fn execute_container(
        &self,
        command: Vec<String>,
        stdin: Option<Vec<u8>>,
    ) -> Result<SandboxResult> {
        // Use Docker/Podman if available
        let container_runtime = if Path::new("/usr/bin/podman").exists() {
            "podman"
        } else if Path::new("/usr/bin/docker").exists() {
            "docker"
        } else {
            return Err(RemoteCacheError::Sandbox(
                "No container runtime found".to_string(),
            ));
        };

        let mut docker_cmd = Command::new(container_runtime);
        docker_cmd.arg("run").arg("--rm").arg("-i");

        // Add volume mounts
        for path in &self.config.read_paths {
            docker_cmd
                .arg("-v")
                .arg(format!("{}:{}:ro", path.display(), path.display()));
        }

        for path in &self.config.write_paths {
            docker_cmd
                .arg("-v")
                .arg(format!("{}:{}:rw", path.display(), path.display()));
        }

        // Set working directory
        docker_cmd.arg("-w").arg(&self.config.working_dir);

        // Add environment variables
        for (key, value) in &self.config.env_vars {
            docker_cmd.arg("-e").arg(format!("{}={}", key, value));
        }

        // Add resource limits
        if let Some(memory) = self.config.memory_limit {
            docker_cmd.arg("-m").arg(memory.to_string());
        }

        if let Some(cpu) = self.config.cpu_limit {
            docker_cmd
                .arg("--cpus")
                .arg((cpu as f64 / 100.0).to_string());
        }

        // Disable network if needed
        if !self.config.allow_network {
            docker_cmd.arg("--network").arg("none");
        }

        // Use a minimal base image
        docker_cmd.arg("alpine:latest");

        // Add the actual command
        docker_cmd.arg("sh").arg("-c").arg(command.join(" "));

        docker_cmd
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let start = std::time::Instant::now();
        let mut child = docker_cmd.spawn()?;

        // Write stdin if provided
        if let Some(input) = stdin {
            if let Some(mut stdin_handle) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin_handle.write_all(&input).await?;
                stdin_handle.flush().await?;
            }
        }

        let output = child.wait_with_output().await?;
        let duration = start.elapsed();

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
            duration,
        })
    }

    /// Prepare sandbox environment
    pub async fn prepare(&self) -> Result<()> {
        // Create working directory
        tokio::fs::create_dir_all(&self.config.working_dir).await?;

        // Set up any necessary mounts or bind points
        match self.config.mode {
            SandboxMode::Container => {
                // Pull container image if needed
                let container_runtime = if Path::new("/usr/bin/podman").exists() {
                    "podman"
                } else if Path::new("/usr/bin/docker").exists() {
                    "docker"
                } else {
                    return Ok(());
                };

                let output = Command::new(container_runtime)
                    .arg("pull")
                    .arg("alpine:latest")
                    .output()
                    .await?;

                if !output.status.success() {
                    return Err(RemoteCacheError::Sandbox(format!(
                        "Failed to pull container image: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )));
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Clean up sandbox environment
    pub async fn cleanup(&self) -> Result<()> {
        // Remove working directory if it's a temporary one
        if self.config.working_dir.starts_with("/tmp/cuenv-sandbox") {
            let _ = tokio::fs::remove_dir_all(&self.config.working_dir).await;
        }

        Ok(())
    }
}

/// Result of sandboxed execution
#[derive(Debug, Clone)]
pub struct SandboxResult {
    /// Exit code
    pub exit_code: i32,
    /// Stdout output
    pub stdout: Vec<u8>,
    /// Stderr output
    pub stderr: Vec<u8>,
    /// Execution duration
    pub duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sandbox_none() {
        let temp_dir = TempDir::new().unwrap();
        let config = SandboxConfig {
            mode: SandboxMode::None,
            working_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let sandbox = Sandbox::new(config);

        // Simple echo command
        let result = sandbox
            .execute(vec!["echo".to_string(), "hello".to_string()], None)
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    }

    #[tokio::test]
    async fn test_sandbox_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let config = SandboxConfig {
            mode: SandboxMode::Basic,
            working_dir: temp_dir.path().to_path_buf(),
            env_vars,
            ..Default::default()
        };

        let sandbox = Sandbox::new(config);

        // Test environment isolation
        let result = sandbox
            .execute(
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo $TEST_VAR".to_string(),
                ],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "test_value");
    }

    #[tokio::test]
    async fn test_sandbox_stdin() {
        let temp_dir = TempDir::new().unwrap();
        let config = SandboxConfig {
            mode: SandboxMode::None,
            working_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let sandbox = Sandbox::new(config);

        // Cat command with stdin
        let input = b"Hello from stdin!";
        let result = sandbox
            .execute(vec!["cat".to_string()], Some(input.to_vec()))
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(&result.stdout, input);
    }

    #[tokio::test]
    async fn test_sandbox_error() {
        let temp_dir = TempDir::new().unwrap();
        let config = SandboxConfig {
            mode: SandboxMode::None,
            working_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let sandbox = Sandbox::new(config);

        // Non-existent command
        let result = sandbox
            .execute(vec!["/nonexistent/command".to_string()], None)
            .await;

        assert!(result.is_err());
    }
}
