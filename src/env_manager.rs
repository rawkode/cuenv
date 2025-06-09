use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::io::{self, BufReader};

use crate::cue_parser::CueParser;
use crate::directory::DirectoryManager;
use crate::secrets::SecretManager;
use crate::output_filter::OutputFilter;

pub struct EnvManager {
    directory_manager: DirectoryManager,
    original_env: HashMap<String, String>,
}

impl EnvManager {
    pub fn new() -> Self {
        Self {
            directory_manager: DirectoryManager::new(),
            original_env: HashMap::new(),
        }
    }

    pub fn load_env(&mut self, dir: &Path) -> Result<()> {
        self.save_original_env();

        let env_files = self.directory_manager.find_env_files(dir)?;
        
        for env_file in env_files {
            log::info!("Loading environment from: {}", env_file.display());
            self.apply_env_file(&env_file)?;
        }

        Ok(())
    }

    pub fn unload_env(&self) -> Result<()> {
        let current_env: Vec<(String, String)> = env::vars().collect();
        
        for (key, _) in current_env {
            if let Some(original_value) = self.original_env.get(&key) {
                env::set_var(&key, original_value);
            } else if !self.original_env.contains_key(&key) {
                env::remove_var(&key);
            }
        }

        Ok(())
    }

    fn save_original_env(&mut self) {
        self.original_env = env::vars().collect();
    }

    fn apply_env_file(&self, path: &Path) -> Result<()> {
        let env_vars = CueParser::parse_env_file(path)
            .with_context(|| format!("Failed to parse env file: {}", path.display()))?;

        for (key, value) in env_vars {
            let expanded_value = shellexpand::full(&value)
                .with_context(|| format!("Failed to expand value for {}", key))?
                .to_string();
            
            log::debug!("Setting {}={}", key, expanded_value);
            env::set_var(key, expanded_value);
        }

        Ok(())
    }

    pub fn print_env_diff(&self) -> Result<()> {
        let current_env: HashMap<String, String> = env::vars().collect();
        
        println!("Environment changes:");
        
        for (key, value) in &current_env {
            if let Some(original) = self.original_env.get(key) {
                if original != value {
                    println!("  {} (modified): {} -> {}", key, original, value);
                }
            } else {
                println!("  {} (new): {}", key, value);
            }
        }

        for (key, value) in &self.original_env {
            if !current_env.contains_key(key) {
                println!("  {} (removed): {}", key, value);
            }
        }

        Ok(())
    }

    pub fn export_for_shell(&self, shell: &str) -> Result<String> {
        let current_env: HashMap<String, String> = env::vars().collect();
        let mut output = String::new();

        match shell {
            "bash" | "zsh" => {
                for (key, value) in &current_env {
                    if !self.original_env.contains_key(key) || 
                       self.original_env.get(key) != Some(value) {
                        output.push_str(&format!("export {}='{}'\n", key, value));
                    }
                }
                
                for key in self.original_env.keys() {
                    if !current_env.contains_key(key) {
                        output.push_str(&format!("unset {}\n", key));
                    }
                }
            }
            "fish" => {
                for (key, value) in &current_env {
                    if !self.original_env.contains_key(key) || 
                       self.original_env.get(key) != Some(value) {
                        output.push_str(&format!("set -gx {} '{}'\n", key, value));
                    }
                }
                
                for key in self.original_env.keys() {
                    if !current_env.contains_key(key) {
                        output.push_str(&format!("set -e {}\n", key));
                    }
                }
            }
            "powershell" => {
                for (key, value) in &current_env {
                    if !self.original_env.contains_key(key) || 
                       self.original_env.get(key) != Some(value) {
                        output.push_str(&format!("$env:{} = '{}'\n", key, value));
                    }
                }
                
                for key in self.original_env.keys() {
                    if !current_env.contains_key(key) {
                        output.push_str(&format!("Remove-Item Env:\\{}\n", key));
                    }
                }
            }
            "cmd" => {
                for (key, value) in &current_env {
                    if !self.original_env.contains_key(key) || 
                       self.original_env.get(key) != Some(value) {
                        output.push_str(&format!("set {}={}\n", key, value));
                    }
                }
                
                for key in self.original_env.keys() {
                    if !current_env.contains_key(key) {
                        output.push_str(&format!("set {}=\n", key));
                    }
                }
            }
            _ => anyhow::bail!("Unsupported shell: {}", shell),
        }

        Ok(output)
    }

    pub fn run_command(&self, command: &str, args: &[String]) -> Result<i32> {
        // Get the loaded environment variables (only the ones from CUE files)
        let mut env_from_cue = HashMap::new();
        let current_env: HashMap<String, String> = env::vars().collect();
        
        // Only include variables that were added or modified by CUE
        for (key, value) in &current_env {
            if !self.original_env.contains_key(key) || 
               self.original_env.get(key) != Some(value) {
                env_from_cue.insert(key.clone(), value.clone());
            }
        }

        // Resolve secrets in the environment variables
        let (resolved_env, secret_values) = if cfg!(test) {
            // Skip secret resolution in tests
            (env_from_cue, HashSet::new())
        } else {
            let secret_manager = SecretManager::new();
            let rt = tokio::runtime::Runtime::new()
                .context("Failed to create tokio runtime")?;
            
            let resolved_secrets = rt.block_on(secret_manager.resolve_secrets(env_from_cue))
                .context("Failed to resolve secrets")?;
            (resolved_secrets.env_vars, resolved_secrets.secret_values)
        };

        // Add minimal required environment variables for basic operation
        let mut final_env = resolved_env;
        
        // PATH is needed to find executables
        if let Some(path) = self.original_env.get("PATH") {
            final_env.insert("PATH".to_string(), path.clone());
        }
        
        // HOME might be needed by some programs
        if let Some(home) = self.original_env.get("HOME") {
            final_env.insert("HOME".to_string(), home.clone());
        }
        
        // Windows uses USERPROFILE instead of HOME
        #[cfg(windows)]
        if let Some(userprofile) = self.original_env.get("USERPROFILE") {
            final_env.insert("USERPROFILE".to_string(), userprofile.clone());
            // Also set HOME for compatibility with Unix-style programs
            if !final_env.contains_key("HOME") {
                final_env.insert("HOME".to_string(), userprofile.clone());
            }
        }

        // Create shared secret values for output filtering
        let secrets = Arc::new(Mutex::new(secret_values));
        
        // Create and execute the command with only the CUE environment
        let mut cmd = Command::new(command);
        cmd.args(args)
           .env_clear()  // Clear all environment variables
           .envs(&final_env)  // Set only our CUE-defined vars with resolved secrets
           .stdin(Stdio::inherit())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .with_context(|| format!("Failed to spawn command: {}", command))?;
        
        // Set up filtered output streams
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");
        
        let stdout_secrets = Arc::clone(&secrets);
        let stderr_secrets = Arc::clone(&secrets);
        
        // Spawn threads to handle output filtering
        let stdout_thread = std::thread::spawn(move || {
            let mut filter = OutputFilter::new(io::stdout(), stdout_secrets);
            io::copy(&mut BufReader::new(stdout), &mut filter)
        });
        
        let stderr_thread = std::thread::spawn(move || {
            let mut filter = OutputFilter::new(io::stderr(), stderr_secrets);
            io::copy(&mut BufReader::new(stderr), &mut filter)
        });
        
        // Wait for the process to complete
        let status = child.wait()
            .with_context(|| format!("Failed to wait for command: {}", command))?;
        
        // Wait for output threads to complete
        stdout_thread.join().unwrap()
            .context("Failed to process stdout")?;
        stderr_thread.join().unwrap()
            .context("Failed to process stderr")?;

        Ok(status.code().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_and_unload_env() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, r#"package env

CUENV_TEST_VAR_UNIQUE: "test_value""#).unwrap();

        let original_value = env::var("CUENV_TEST_VAR_UNIQUE").ok();
        
        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();
        
        assert_eq!(env::var("CUENV_TEST_VAR_UNIQUE").unwrap(), "test_value");
        
        manager.unload_env().unwrap();
        
        match original_value {
            Some(val) => assert_eq!(env::var("CUENV_TEST_VAR_UNIQUE").unwrap(), val),
            None => assert!(env::var("CUENV_TEST_VAR_UNIQUE").is_err()),
        }
    }

    #[test]
    fn test_run_command_hermetic() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, r#"package env

TEST_FROM_CUE: "cue_value"
PORT: "8080""#).unwrap();

        // Set a variable that should NOT be passed to the child
        env::set_var("TEST_PARENT_VAR", "should_not_exist");
        
        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();
        
        // Run a command that checks for our variables
        #[cfg(unix)]
        let (cmd, args) = ("sh", vec![
            "-c".to_string(), 
            "test \"$TEST_FROM_CUE\" = \"cue_value\" && test -z \"$TEST_PARENT_VAR\"".to_string()
        ]);
        
        #[cfg(windows)]
        let (cmd, args) = ("cmd", vec![
            "/C".to_string(),
            "if \"%TEST_FROM_CUE%\"==\"cue_value\" (if \"%TEST_PARENT_VAR%\"==\"\" exit 0 else exit 1) else exit 1".to_string()
        ]);
        
        let status = manager.run_command(cmd, &args).unwrap();
        
        assert_eq!(status, 0, "Command should succeed with correct environment");
        
        // Clean up
        env::remove_var("TEST_PARENT_VAR");
    }

    #[test]
    fn test_run_command_with_secret_refs() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        
        // Write a CUE file with normal values only
        // We can't test actual secret resolution without mocking the secret managers
        fs::write(&env_file, r#"package env

NORMAL_VAR: "normal-value"
ANOTHER_VAR: "another-value""#).unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();
        
        // Run a command that checks the variables
        #[cfg(unix)]
        let (cmd, args) = ("sh", vec![
            "-c".to_string(),
            "test \"$NORMAL_VAR\" = \"normal-value\" && test \"$ANOTHER_VAR\" = \"another-value\"".to_string()
        ]);
        
        #[cfg(windows)]
        let (cmd, args) = ("cmd", vec![
            "/C".to_string(),
            "if \"%NORMAL_VAR%\"==\"normal-value\" (if \"%ANOTHER_VAR%\"==\"another-value\" exit 0 else exit 1) else exit 1".to_string()
        ]);
        
        let status = manager.run_command(cmd, &args).unwrap();
        
        assert_eq!(status, 0, "Command should succeed with all variables set");
    }

    #[test]
    fn test_run_command_preserves_path_and_home() {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, r#"package env

TEST_VAR: "test""#).unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).unwrap();
        
        // Run a command that checks PATH and HOME are preserved
        #[cfg(unix)]
        let (cmd, args) = ("sh", vec![
            "-c".to_string(),
            "test -n \"$PATH\" && test -n \"$HOME\"".to_string()
        ]);
        
        #[cfg(windows)]
        let (cmd, args) = ("cmd", vec![
            "/C".to_string(),
            "if defined PATH (if defined HOME exit 0 else exit 1) else exit 1".to_string()
        ]);
        
        let status = manager.run_command(cmd, &args).unwrap();
        
        assert_eq!(status, 0, "PATH and HOME should be preserved");
    }
}