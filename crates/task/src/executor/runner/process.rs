use cuenv_core::{Result, TaskDefinition, TaskExecutionMode};
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};

/// Execute a single task
pub async fn execute_single_task(
    task_name: &str,
    task_definition: &TaskDefinition,
    _working_dir: &Path,
    args: &[String],
    audit_mode: bool,
    capture_output: bool,
) -> Result<i32> {
    // Determine what to execute from TaskDefinition
    let (shell, script_content) = match &task_definition.execution_mode {
        TaskExecutionMode::Command { command } => {
            // Add user args to the command
            let full_command = if args.is_empty() {
                command.clone()
            } else {
                format!("{} {}", command, args.join(" "))
            };
            (task_definition.shell.clone(), full_command)
        }
        TaskExecutionMode::Script { content } => (task_definition.shell.clone(), content.clone()),
    };

    // Validate for security
    validate_security(&shell, &script_content, args)?;

    // Use the working directory from task definition
    let exec_dir = task_definition.working_directory.clone();

    // Configure command
    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(&script_content).current_dir(&exec_dir);

    configure_stdio(&mut cmd, capture_output);
    configure_platform_specific(&mut cmd);

    // Apply security restrictions if configured
    if let Some(security) = &task_definition.security {
        if let Some(exit_code) =
            super::security::apply_security_restrictions(&mut cmd, security, audit_mode)?
        {
            return Ok(exit_code);
        }
    }

    // Execute with output handling
    super::output::execute_with_output_handling(
        cmd,
        &shell,
        script_content,
        task_definition.timeout,
        task_name,
        capture_output,
    )
    .await
}

fn validate_security(shell: &str, script_content: &str, args: &[String]) -> Result<()> {
    // Use a static set for allowed shells to avoid repeated allocations
    static ALLOWED_SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "pwsh", "powershell"];
    let allowed_shells: HashSet<String> = ALLOWED_SHELLS.iter().map(|&s| s.to_string()).collect();

    cuenv_security::SecurityValidator::validate_command(shell, &allowed_shells)?;
    cuenv_security::SecurityValidator::validate_shell_expansion(script_content)?;

    if !args.is_empty() {
        cuenv_security::SecurityValidator::validate_command_args(args)?;
    }

    Ok(())
}

fn configure_stdio(cmd: &mut Command, capture_output: bool) {
    if capture_output {
        // Capture output for TUI mode to prevent interference
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    } else {
        // Normal mode - inherit stdio
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    }
}

fn configure_platform_specific(cmd: &mut Command) {
    // On Unix, create a new process group for better cleanup
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);

        // Apply resource limits before spawning
        unsafe {
            cmd.pre_exec(|| {
                // TODO: Add apply_default_limits when moved to workspace
                match Ok::<(), std::io::Error>(()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        tracing::warn!("Failed to apply resource limits: {}", e);
                        Ok(()) // Continue anyway
                    }
                }
            });
        }
    }
}
