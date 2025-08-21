use cuenv_core::{Error, Result};
use cuenv_utils::cleanup::handler::ProcessGuard;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Execute command with output handling
pub async fn execute_with_output_handling(
    mut cmd: Command,
    shell: &str,
    script_content: String,
    timeout: Duration,
    task_name: &str,
    capture_output: bool,
) -> Result<i32> {
    // Spawn the process with timeout
    let mut child = cmd.spawn().map_err(|e| {
        Error::command_execution(
            shell,
            vec!["-c".to_string(), script_content.clone()],
            format!("Failed to spawn task: {e}"),
            None,
        )
    })?;

    // Handle output capturing if needed
    let (stdout_handle, stderr_handle, captured_output) = if capture_output {
        let output = Arc::new(Mutex::new(CapturedOutput::default()));
        let task_name_clone = task_name.to_string();
        let (stdout_h, stderr_h) =
            handle_captured_output(&mut child, &task_name_clone, Arc::clone(&output));
        (stdout_h, stderr_h, Some(output))
    } else {
        (None, None, None)
    };

    // Use ProcessGuard for automatic cleanup
    let mut guard = ProcessGuard::new(child, timeout);

    // Wait for completion with timeout (use async version to avoid blocking the runtime)
    let status = guard.wait_with_timeout_async().await.map_err(|e| {
        Error::command_execution(
            shell,
            vec!["-c".to_string(), script_content.clone()],
            e.to_string(),
            None,
        )
    })?;

    // Wait for output threads to complete
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    let exit_code = status.code().unwrap_or(1);

    // If the task failed and we captured output, send it through the event system
    // This ensures TUI can display it properly without corrupting the terminal
    if exit_code != 0 {
        if let Some(output) = captured_output {
            // Extract the captured output to avoid holding the lock across await
            let (stdout_lines, stderr_lines) = {
                if let Ok(captured) = output.lock() {
                    (captured.stdout.clone(), captured.stderr.clone())
                } else {
                    (vec![], vec![])
                }
            };

            if !stdout_lines.is_empty() || !stderr_lines.is_empty() {
                // Send output through tracing system for proper handling

                // Send stdout as tracing events
                if !stdout_lines.is_empty() {
                    let combined_stdout = stdout_lines.join("\n");
                    tracing::info!(
                        task_name = %task_name,
                        task_id = %task_name,
                        output = %combined_stdout,
                        event_type = "output",
                        "task_output"
                    );
                }

                // Send stderr as tracing events
                if !stderr_lines.is_empty() {
                    let combined_stderr = stderr_lines.join("\n");
                    tracing::warn!(
                        task_name = %task_name,
                        task_id = %task_name,
                        error = %combined_stderr,
                        event_type = "error_output",
                        "task_error"
                    );
                }
            }
        }
    }

    Ok(exit_code)
}

#[derive(Default)]
struct CapturedOutput {
    stdout: Vec<String>,
    stderr: Vec<String>,
}

fn handle_captured_output(
    child: &mut std::process::Child,
    _task_name: &str,
    captured_output: Arc<Mutex<CapturedOutput>>,
) -> (
    Option<std::thread::JoinHandle<()>>,
    Option<std::thread::JoinHandle<()>>,
) {
    use std::io::{BufRead, BufReader};

    // Take stdout and stderr from child
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn thread to read stdout
    let stdout_handle = stdout.map(|stdout| {
        let output_clone = Arc::clone(&captured_output);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|result| result.ok()) {
                // Store for potential error display
                if let Ok(mut output) = output_clone.lock() {
                    output.stdout.push(line);
                }
                // Note: Real-time event sending removed as it's not working reliably
                // Events will be sent after task completion
            }
        })
    });

    // Spawn thread to read stderr
    let stderr_handle = stderr.map(|stderr| {
        let output_clone = Arc::clone(&captured_output);
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|result| result.ok()) {
                // Store for potential error display
                if let Ok(mut output) = output_clone.lock() {
                    output.stderr.push(line);
                }
                // Note: Real-time event sending removed as it's not working reliably
                // Events will be sent after task completion
            }
        })
    });

    (stdout_handle, stderr_handle)
}
