//! Task Server Protocol implementation
//!
//! This module implements the devenv Task Server Protocol (TSP) - a JSON-RPC based
//! protocol that allows external tools to register tasks with cuenv through a 
//! long-running server process.
//!
//! Protocol specification:
//! - External tools act as JSON-RPC servers exposing tasks via Unix domain sockets
//! - cuenv discovers servers by launching executables (like `myexecutable /tmp/socket.sock`)
//! - Communication uses JSON-RPC 2.0 with initialize and run methods
//! - Servers stream back log output and final results

use cuenv_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Child;
use tokio::time::timeout;

/// JSON-RPC 2.0 request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    method: String,
    params: T,
    id: u64,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<JsonRpcError>,
    id: u64,
}

/// JSON-RPC error structure
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

/// Initialize request parameters (empty)
#[derive(Debug, Serialize)]
struct InitializeParams {}

/// Initialize response containing available tasks
#[derive(Debug, Deserialize)]
struct InitializeResult {
    tasks: Vec<TaskDefinition>,
}

/// Task definition from server
#[derive(Debug, Deserialize)]
struct TaskDefinition {
    name: String,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Run task request parameters
#[derive(Debug, Serialize)]
struct RunTaskParams {
    task: String,
    #[serde(default)]
    inputs: HashMap<String, String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

/// Run task response
#[derive(Debug, Deserialize)]
struct RunTaskResult {
    exit_code: i32,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

/// Log message from server
#[derive(Debug, Deserialize)]
struct LogMessage {
    task: String,
    stream: String, // "stdout" or "stderr"
    content: String,
}

/// Task server client that communicates with external task servers
pub struct TaskServerClient {
    socket_path: PathBuf,
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
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                Error::file_system(
                    self.socket_path.clone(),
                    "remove existing socket",
                    e,
                )
            })?;
        }

        // Launch server process
        let mut cmd = Command::new(executable);
        cmd.arg(&self.socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            Error::command_execution(
                executable,
                vec![self.socket_path.to_string_lossy().to_string()],
                format!("Failed to launch task server: {}", e),
                None,
            )
        })?;

        // Wait for socket to be created (with timeout)
        let socket_ready = timeout(Duration::from_secs(10), async {
            while !self.socket_path.exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await;

        if socket_ready.is_err() {
            // Try to kill the child process
            let _ = child.kill().await;
            return Err(Error::configuration(
                "Timeout waiting for task server to create socket".to_string(),
            ));
        }

        // Connect to socket
        let stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            Error::configuration(format!(
                "Failed to connect to task server socket: {}",
                e
            ))
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

        let result = response.result.ok_or_else(|| {
            Error::configuration("Task server returned no result".to_string())
        })?;

        Ok(result.tasks)
    }

    /// Run a task on the server
    pub async fn run_task(
        &mut self,
        task_name: &str,
        inputs: HashMap<String, String>,
        outputs: HashMap<String, String>,
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

        let result = response.result.ok_or_else(|| {
            Error::configuration("Task server returned no result".to_string())
        })?;

        Ok(result)
    }

    /// Send JSON-RPC request and wait for response
    async fn send_request<T: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        request: JsonRpcRequest<T>,
    ) -> Result<JsonRpcResponse<R>> {
        let stream = self.stream.as_mut().ok_or_else(|| {
            Error::configuration("Not connected to task server".to_string())
        })?;

        // Serialize request
        let request_json = serde_json::to_string(&request).map_err(|e| {
            Error::configuration(format!("Failed to serialize request: {}", e))
        })?;

        // Send request
        stream
            .write_all(format!("{}\n", request_json).as_bytes())
            .await
            .map_err(|e| {
                Error::configuration(format!("Failed to send request: {}", e))
            })?;

        // Read response
        let mut buf_reader = BufReader::new(stream);
        let mut response_line = String::new();
        buf_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| {
                Error::configuration(format!("Failed to read response: {}", e))
            })?;

        // Parse response
        let response: JsonRpcResponse<R> = serde_json::from_str(&response_line).map_err(|e| {
            Error::configuration(format!(
                "Failed to parse response: {} - Response: {}",
                e, response_line
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
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                Error::file_system(
                    self.socket_path.clone(),
                    "remove socket file",
                    e,
                )
            })?;
        }

        Ok(())
    }
}

impl Drop for TaskServerClient {
    fn drop(&mut self) {
        // Best effort cleanup
        if let Some(mut process) = self.server_process.take() {
            let _ = std::process::Command::new("kill")
                .arg(process.id().to_string())
                .output();
        }

        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// Task server manager that handles multiple external task servers
pub struct TaskServerManager {
    servers: Vec<TaskServerClient>,
    socket_dir: PathBuf,
}

impl TaskServerManager {
    /// Create a new task server manager
    pub fn new(socket_dir: PathBuf) -> Self {
        Self {
            servers: Vec::new(),
            socket_dir,
        }
    }

    /// Add a task server by launching an executable
    pub async fn add_server(&mut self, executable: &str, server_name: &str) -> Result<Vec<TaskDefinition>> {
        // Create socket path
        let socket_path = self.socket_dir.join(format!("{}.sock", server_name));
        
        let mut client = TaskServerClient::new(socket_path);
        
        // Launch and connect
        client.launch_and_connect(executable).await?;
        
        // Initialize and get tasks
        let tasks = client.initialize().await?;
        
        self.servers.push(client);
        
        Ok(tasks)
    }

    /// Discover task servers from a directory
    pub async fn discover_servers(&mut self, discovery_dir: &Path) -> Result<Vec<TaskDefinition>> {
        let mut all_tasks = Vec::new();
        
        if !discovery_dir.exists() {
            return Ok(all_tasks);
        }

        // Look for executable files in the discovery directory
        let entries = std::fs::read_dir(discovery_dir).map_err(|e| {
            Error::file_system(
                discovery_dir.to_path_buf(),
                "read discovery directory",
                e,
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                Error::file_system(
                    discovery_dir.to_path_buf(),
                    "read directory entry",
                    e,
                )
            })?;

            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    // Try to add this as a task server
                    match self.add_server(path.to_str().unwrap(), name).await {
                        Ok(mut tasks) => {
                            // Prefix task names with server name
                            for task in &mut tasks {
                                task.name = format!("{}:{}", name, task.name);
                            }
                            all_tasks.extend(tasks);
                        }
                        Err(e) => {
                            // Log error but continue with other servers
                            tracing::warn!(
                                server = %path.display(),
                                error = %e,
                                "Failed to connect to task server"
                            );
                        }
                    }
                }
            }
        }

        Ok(all_tasks)
    }

    /// Run a task on the appropriate server
    pub async fn run_task(
        &mut self,
        task_name: &str,
        inputs: HashMap<String, String>,
        outputs: HashMap<String, String>,
    ) -> Result<i32> {
        // For now, just try the first server
        // TODO: Implement proper task routing based on task name prefix
        if let Some(server) = self.servers.first_mut() {
            let result = server.run_task(task_name, inputs, outputs).await?;
            Ok(result.exit_code)
        } else {
            Err(Error::configuration(
                "No task servers available".to_string(),
            ))
        }
    }

    /// Shutdown all servers
    pub async fn shutdown(&mut self) -> Result<()> {
        for server in &mut self.servers {
            server.shutdown().await?;
        }
        self.servers.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_task_server_client_creation() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");
        
        let client = TaskServerClient::new(socket_path.clone());
        assert_eq!(client.socket_path, socket_path);
        assert!(client.stream.is_none());
        assert!(client.server_process.is_none());
    }

    #[tokio::test]
    async fn test_task_server_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let manager = TaskServerManager::new(temp_dir.path().to_path_buf());
        assert_eq!(manager.socket_dir, temp_dir.path());
        assert!(manager.servers.is_empty());
    }

    #[tokio::test]
    async fn test_discover_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let socket_dir = temp_dir.path().join("sockets");
        fs::create_dir_all(&socket_dir).unwrap();
        
        let mut manager = TaskServerManager::new(socket_dir);
        let tasks = manager.discover_servers(temp_dir.path()).await.unwrap();
        assert!(tasks.is_empty());
    }
}