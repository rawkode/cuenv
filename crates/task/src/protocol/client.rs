//! Task server client that communicates with external task servers

use super::types::{
    InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse, RunTaskParams,
    RunTaskResult, TaskDefinition,
};
use cuenv_core::{Error, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::time::timeout;

/// Task server client that communicates with external task servers
pub struct TaskServerClient {
    pub(crate) socket_path: PathBuf,
    server_process: Option<Child>,
    stream: Option<UnixStream>,
    next_id: u64,
}

impl TaskServerClient {
    /// Create a new task server client
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            server_process: None,
            stream: None,
            next_id: 1,
        }
    }

    /// Launch external server process and connect
    pub async fn launch_and_connect(&mut self, executable: &str) -> Result<()> {
        // Remove socket if it exists
        if self.socket_path.exists() {
            tokio::fs::remove_file(&self.socket_path)
                .await
                .map_err(|e| {
                    Error::file_system(self.socket_path.clone(), "remove existing socket", e)
                })?;
        }

        // Launch server process
        let mut cmd = Command::new(executable);
        cmd.arg(&self.socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            Error::command_execution(
                executable,
                vec![self.socket_path.to_string_lossy().to_string()],
                format!("Failed to launch task server: {e}"),
                None,
            )
        })?;

        // Wait for socket to be created (with timeout)
        let socket_ready = timeout(Duration::from_secs(10), async {
            while !self.socket_path.exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        if socket_ready.is_err() {
            // Try to kill the child process
            let _ = tokio::process::Command::new("kill")
                .arg(format!("{}", child.id().unwrap_or(0)))
                .output()
                .await;
            return Err(Error::configuration(
                "Timeout waiting for task server to create socket".to_string(),
            ));
        }

        // Connect to socket
        let stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            Error::configuration(format!("Failed to connect to task server socket: {e}"))
        })?;

        self.server_process = Some(child);
        self.stream = Some(stream);

        Ok(())
    }

    /// Initialize connection with server and get available tasks
    pub async fn initialize(&mut self) -> Result<Vec<TaskDefinition>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: InitializeParams {},
            id: self.next_id(),
        };

        let response: JsonRpcResponse<InitializeResult> = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::configuration(format!(
                "Task server initialization failed: {} (code {})",
                error.message, error.code
            )));
        }

        let result = response
            .result
            .ok_or_else(|| Error::configuration("Task server returned no result".to_string()))?;

        Ok(result.tasks)
    }

    /// Run a task on the server
    pub async fn run_task(
        &mut self,
        task_name: &str,
        inputs: std::collections::HashMap<String, String>,
        outputs: std::collections::HashMap<String, String>,
    ) -> Result<RunTaskResult> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "run".to_string(),
            params: RunTaskParams {
                task: task_name.to_string(),
                inputs,
                outputs,
            },
            id: self.next_id(),
        };

        let response: JsonRpcResponse<RunTaskResult> = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::configuration(format!(
                "Task execution failed: {} (code {})",
                error.message, error.code
            )));
        }

        let result = response
            .result
            .ok_or_else(|| Error::configuration("Task server returned no result".to_string()))?;

        Ok(result)
    }

    /// Send JSON-RPC request and wait for response
    async fn send_request<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(
        &mut self,
        request: JsonRpcRequest<T>,
    ) -> Result<JsonRpcResponse<R>> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| Error::configuration("Not connected to task server".to_string()))?;

        // Serialize request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| Error::configuration(format!("Failed to serialize request: {e}")))?;

        // Send request
        stream
            .write_all(format!("{request_json}\n").as_bytes())
            .await
            .map_err(|e| Error::configuration(format!("Failed to send request: {e}")))?;

        // Read response
        let mut buf_reader = BufReader::new(stream);
        let mut response_line = String::new();
        buf_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read response: {e}")))?;

        // Parse response
        let response: JsonRpcResponse<R> = serde_json::from_str(&response_line).map_err(|e| {
            Error::configuration(format!(
                "Failed to parse response: {e} - Response: {response_line}"
            ))
        })?;

        Ok(response)
    }

    /// Get next request ID
    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Shutdown the server and cleanup
    pub async fn shutdown(&mut self) -> Result<()> {
        // Close stream
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Kill server process if still running
        if let Some(mut process) = self.server_process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }

        // Remove socket file
        if self.socket_path.exists() {
            tokio::fs::remove_file(&self.socket_path)
                .await
                .map_err(|e| {
                    Error::file_system(self.socket_path.clone(), "remove socket file", e)
                })?;
        }

        Ok(())
    }
}

impl Drop for TaskServerClient {
    fn drop(&mut self) {
        // Best effort cleanup
        if let Some(process) = self.server_process.take() {
            if let Some(pid) = process.id() {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }
        }

        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}