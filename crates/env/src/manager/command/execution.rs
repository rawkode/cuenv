use cuenv_core::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};

use super::output::wait_for_output_threads;
use crate::manager::secrets::resolve_secret;
use crate::manager::stubs::{OutputFilter, Platform};

/// Setup environment variables for command execution
pub fn setup_command_environment(
    sourced_env: &HashMap<String, String>,
    cue_vars: &HashMap<String, String>,
    original_env: &HashMap<String, String>,
) -> HashMap<String, String> {
    // Start with sourced environment (from nix, devenv, etc.)
    let mut base_env = sourced_env.clone();

    // Override with CUE-defined variables (CUE takes precedence)
    base_env.extend(cue_vars.clone());

    // Resolve secrets in the merged environment
    let mut resolved_env = HashMap::new();
    for (key, value) in base_env {
        let resolved_value = match resolve_secret(&value) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to resolve secret for {}: {}", key, e);
                value // Keep original value if resolution fails
            }
        };
        resolved_env.insert(key, resolved_value);
    }

    // Add minimal required environment variables for basic operation
    let mut final_env = resolved_env;

    // PATH is needed to find executables - use sourced PATH if available, fallback to original
    if !final_env.contains_key("PATH") {
        if let Some(path) = original_env.get("PATH") {
            final_env.insert("PATH".to_string(), path.clone());
        }
    }

    // Set up platform-specific environment
    Platform::setup_environment(&mut final_env);

    // Ensure HOME directory is available (platform-specific)
    let home_var = Platform::home_env_var();
    if let Some(home_value) = original_env.get(home_var) {
        final_env.insert(home_var.to_string(), home_value.clone());
    }

    // Ensure HOME is set on all platforms for compatibility
    if let Some(home) = original_env.get("HOME") {
        final_env.insert("HOME".to_string(), home.clone());
    }

    final_env
}

/// Execute command and handle output
pub fn execute_command(
    command: &str,
    args: &[String],
    final_env: HashMap<String, String>,
) -> Result<i32> {
    // Create shared secret values for output filtering
    let secret_set = HashSet::new();
    let secrets = Arc::new(RwLock::new(secret_set));

    // Create and execute the command with only the CUE environment
    let mut cmd = Command::new(command);
    cmd.args(args)
        .env_clear() // Clear all environment variables
        .envs(&final_env) // Set only our CUE-defined vars with resolved secrets
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Configure process group for better cleanup on Unix
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                format!("Failed to spawn command: {e}"),
                None,
            ));
        }
    };

    // Set up filtered output streams
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                "Failed to capture stdout".to_string(),
                None,
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                "Failed to capture stderr".to_string(),
                None,
            ));
        }
    };

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
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                format!("Failed to wait for command: {e}"),
                None,
            ));
        }
    };

    // Wait for output threads to complete
    wait_for_output_threads(stdout_thread, stderr_thread, command, args, status.code())?;

    Ok(status.code().unwrap_or(1))
}
