use cuenv_core::{Error, Result};
use cuenv_utils::cleanup::handler::ProcessGuard;
use std::process::Command;
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

    // Handle output streaming if capturing
    let (stdout_handle, stderr_handle) = if capture_output {
        handle_captured_output(&mut child, task_name)
    } else {
        (None, None)
    };

    // Use ProcessGuard for automatic cleanup
    let mut guard = ProcessGuard::new(child, timeout);

    // Wait for completion with timeout (use async version to avoid blocking the runtime)
    let status = guard.wait_with_timeout_async().await.map_err(|e| {
        Error::command_execution(
            shell,
            vec!["-c".to_string(), script_content],
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

    Ok(status.code().unwrap_or(1))
}

fn handle_captured_output(
    child: &mut std::process::Child,
    task_name: &str,
) -> (
    Option<std::thread::JoinHandle<()>>,
    Option<std::thread::JoinHandle<()>>,
) {
    use std::io::{BufRead, BufReader};

    // Take stdout and stderr from child
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let _task_name_stdout = task_name.to_string();
    let _task_name_stderr = task_name.to_string();

    // Spawn thread to read stdout
    let stdout_handle = stdout.map(|stdout| {
        std::thread::spawn(move || {
            // Create a tokio runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            if let Ok(_rt) = rt {
                let reader = BufReader::new(stdout);
                for _line in reader.lines().map_while(|result| result.ok()) {
                    // Try to publish to event bus
                    // TODO: Add event publishing through proper abstraction
                    if false {
                        // let task_name = task_name_stdout.clone();
                        // let event = TaskEvent::Log {
                        //     task_name,
                        //     stream: LogStream::Stdout,
                        //     content: line,
                        // };
                        //
                        // // Use spawn instead of block_on to avoid blocking the runtime
                        // let event_bus_clone = event_bus.clone();
                        // rt.spawn(async move {
                        //     event_bus_clone.publish(event).await;
                        // });
                    }
                }
            }
        })
    });

    // Spawn thread to read stderr
    let stderr_handle = stderr.map(|stderr| {
        std::thread::spawn(move || {
            // Create a tokio runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            if let Ok(_rt) = rt {
                let reader = BufReader::new(stderr);
                for _line in reader.lines().map_while(|result| result.ok()) {
                    // Try to publish to event bus
                    // TODO: Add event publishing through proper abstraction
                    if false {
                        // let task_name = task_name_stderr.clone();
                        // let event = TaskEvent::Log {
                        //     task_name,
                        //     stream: LogStream::Stderr,
                        //     content: line,
                        // };
                        //
                        // // Use spawn instead of block_on to avoid blocking the runtime
                        // let event_bus_clone = event_bus.clone();
                        // rt.spawn(async move {
                        //     event_bus_clone.publish(event).await;
                        // });
                    }
                }
            }
        })
    });

    (stdout_handle, stderr_handle)
}
