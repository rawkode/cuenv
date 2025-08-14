use cucumber::World;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Test world context for BDD scenarios
#[derive(Debug, Default, World)]
pub struct TestWorld {
    /// Temporary directory for test isolation
    pub temp_dir: Option<TempDir>,

    /// Current working directory for the test
    pub working_dir: Option<PathBuf>,

    /// Environment variables set during the test
    pub env_vars: HashMap<String, String>,

    /// Background processes started during the test
    pub processes: Vec<Child>,

    /// Current shell being tested
    pub shell: Option<String>,

    /// Hook outputs captured during test
    pub hook_outputs: Vec<String>,

    /// Tasks executed during the test
    pub executed_tasks: Vec<String>,

    /// Secrets resolved during the test
    pub resolved_secrets: HashMap<String, String>,

    /// Exit code from last command
    pub last_exit_code: Option<i32>,

    /// Output from last command
    pub last_output: Option<String>,

    /// Error from last command
    pub last_error: Option<String>,

    /// Duration of last command execution
    pub last_command_duration: Option<Duration>,
}

impl TestWorld {
    /// Create a new test directory with isolation
    pub fn setup_test_dir(&mut self) -> anyhow::Result<PathBuf> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_path_buf();
        self.working_dir = Some(path.clone());
        self.temp_dir = Some(temp_dir);
        Ok(path)
    }

    /// Write a CUE file to the test directory
    pub fn write_cue_file(&self, filename: &str, content: &str) -> anyhow::Result<()> {
        let path = self
            .working_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
            .join(filename);
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Execute a cuenv command in the test directory
    pub async fn run_cuenv(&mut self, args: &[&str]) -> anyhow::Result<()> {
        // Find the cuenv binary in the target directory
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let cuenv_path = std::path::Path::new(&manifest_dir)
            .join("target")
            .join("debug")
            .join("cuenv");

        if !cuenv_path.exists() {
            anyhow::bail!(
                "cuenv binary not found at {:?}. Please run 'cargo build' first.",
                cuenv_path
            );
        }

        let start_time = Instant::now();
        let output = std::process::Command::new(cuenv_path)
            .args(args)
            .current_dir(self.working_dir.as_ref().unwrap())
            .envs(&self.env_vars)
            .output()?;
        let duration = start_time.elapsed();

        self.last_exit_code = Some(output.status.code().unwrap_or(-1));
        self.last_output = Some(String::from_utf8_lossy(&output.stdout).to_string());
        self.last_error = Some(String::from_utf8_lossy(&output.stderr).to_string());
        self.last_command_duration = Some(duration);

        Ok(())
    }

    /// Clean up any background processes
    pub fn cleanup(&mut self) {
        for mut process in self.processes.drain(..) {
            let _ = process.kill();
        }
    }
}

impl Drop for TestWorld {
    fn drop(&mut self) {
        self.cleanup();
    }
}
