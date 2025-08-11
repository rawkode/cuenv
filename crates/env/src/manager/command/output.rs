use cuenv_core::{Error, Result};
use std::io;

/// Wait for output threads to complete
pub fn wait_for_output_threads(
    stdout_thread: std::thread::JoinHandle<io::Result<u64>>,
    stderr_thread: std::thread::JoinHandle<io::Result<u64>>,
    command: &str,
    args: &[String],
    status_code: Option<i32>,
) -> Result<()> {
    match stdout_thread.join() {
        Ok(result) => match result {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    format!("Failed to process stdout: {e}"),
                    status_code,
                ));
            }
        },
        Err(_) => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                "stdout thread panicked".to_string(),
                status_code,
            ));
        }
    }

    match stderr_thread.join() {
        Ok(result) => match result {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::command_execution(
                    command,
                    args.to_vec(),
                    format!("Failed to process stderr: {e}"),
                    status_code,
                ));
            }
        },
        Err(_) => {
            return Err(Error::command_execution(
                command,
                args.to_vec(),
                "stderr thread panicked".to_string(),
                status_code,
            ));
        }
    }

    Ok(())
}