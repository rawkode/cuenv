mod execution;
mod output;

use cuenv_core::Result;
use std::collections::HashMap;

use crate::manager::stubs::AccessRestrictions;
use execution::{execute_command, execute_command_direct, setup_command_environment};
use output::wait_for_output_threads;

/// Run a command with the configured environment
pub fn run_command(
    command: &str,
    args: &[String],
    sourced_env: &HashMap<String, String>,
    cue_vars: &HashMap<String, String>,
    original_env: &HashMap<String, String>,
) -> Result<i32> {
    let final_env = setup_command_environment(sourced_env, cue_vars, original_env);
    execute_command(command, args, final_env)
}

/// Run a command with direct stdio inheritance (for exec command)
pub fn run_command_direct(
    command: &str,
    args: &[String],
    sourced_env: &HashMap<String, String>,
    cue_vars: &HashMap<String, String>,
    original_env: &HashMap<String, String>,
) -> Result<i32> {
    let final_env = setup_command_environment(sourced_env, cue_vars, original_env);
    execute_command_direct(command, args, final_env)
}

/// Run a command with access restrictions in a hermetic environment
pub fn run_command_with_restrictions(
    command: &str,
    args: &[String],
    restrictions: &AccessRestrictions,
    sourced_env: &HashMap<String, String>,
    cue_vars: &HashMap<String, String>,
    original_env: &HashMap<String, String>,
) -> Result<i32> {
    let final_env = setup_command_environment(sourced_env, cue_vars, original_env);

    // Create and execute the command with only the CUE environment
    let mut cmd = std::process::Command::new(command);
    cmd.args(args)
        .env_clear() // Clear all environment variables
        .envs(&final_env) // Set only our CUE-defined vars with resolved secrets
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Apply access restrictions before spawning the process
    restrictions.apply_to_command(&mut cmd)?;

    // Use the common execution logic with the modified command
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Err(cuenv_core::Error::command_execution(
                command,
                args.to_vec(),
                format!("Failed to spawn command: {e}"),
                None,
            ));
        }
    };

    // Handle output streams
    use std::collections::HashSet;
    use std::io::{self, BufReader};
    use std::sync::{Arc, RwLock};

    use crate::manager::stubs::OutputFilter;

    let secret_set = HashSet::new();
    let secrets = Arc::new(RwLock::new(secret_set));

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            return Err(cuenv_core::Error::command_execution(
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
            return Err(cuenv_core::Error::command_execution(
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
            return Err(cuenv_core::Error::command_execution(
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
