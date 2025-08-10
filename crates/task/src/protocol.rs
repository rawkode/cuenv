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

use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Child;
use tokio::process::Command;
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
#[allow(dead_code)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<JsonRpcError>,
    id: u64,
}

/// JSON-RPC error structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub name: String,
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
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
#[allow(dead_code)]
pub struct RunTaskResult {
    exit_code: i32,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

/// Log message from server
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
                format!("Failed to launch task server: {}", e),
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
            Error::configuration(format!("Failed to connect to task server socket: {}", e))
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

        let result = response
            .result
            .ok_or_else(|| Error::configuration("Task server returned no result".to_string()))?;

        Ok(result)
    }

    /// Send JSON-RPC request and wait for response
    async fn send_request<T: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        request: JsonRpcRequest<T>,
    ) -> Result<JsonRpcResponse<R>> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| Error::configuration("Not connected to task server".to_string()))?;

        // Serialize request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| Error::configuration(format!("Failed to serialize request: {}", e)))?;

        // Send request
        stream
            .write_all(format!("{}\n", request_json).as_bytes())
            .await
            .map_err(|e| Error::configuration(format!("Failed to send request: {}", e)))?;

        // Read response
        let mut buf_reader = BufReader::new(stream);
        let mut response_line = String::new();
        buf_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read response: {}", e)))?;

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
    pub async fn add_server(
        &mut self,
        executable: &str,
        server_name: &str,
    ) -> Result<Vec<TaskDefinition>> {
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
            Error::file_system(discovery_dir.to_path_buf(), "read discovery directory", e)
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                Error::file_system(discovery_dir.to_path_buf(), "read directory entry", e)
            })?;

            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    // Try to add this as a task server
                    match self.add_server(&path.to_string_lossy(), name).await {
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
        // Route task to the correct server based on prefix
        let (server_prefix, actual_task_name) = if let Some(idx) = task_name.find(':') {
            let (prefix, rest) = task_name.split_at(idx);
            (Some(prefix), &rest[1..])
        } else {
            (None, task_name)
        };

        if let Some(prefix) = server_prefix {
            // Find server by prefix
            let server_index = self.servers.iter().position(|server| {
                // Extract server name from socket path for comparison
                server
                    .socket_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|name| name.trim_end_matches(".sock") == prefix)
                    .unwrap_or(false)
            });

            if let Some(idx) = server_index {
                let result = self.servers[idx]
                    .run_task(actual_task_name, inputs, outputs)
                    .await?;
                Ok(result.exit_code)
            } else {
                Err(Error::configuration(format!(
                    "No task server found for prefix '{}'",
                    prefix
                )))
            }
        } else if let Some(server) = self.servers.first_mut() {
            // Fallback: no prefix, use first server
            let result = server.run_task(actual_task_name, inputs, outputs).await?;
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

/// Task server provider that exposes cuenv tasks to external tools
pub struct TaskServerProvider {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
    tasks: Arc<HashMap<String, TaskConfig>>,
}

impl TaskServerProvider {
    /// Create a new task server provider
    pub fn new(socket_path: PathBuf, tasks: HashMap<String, TaskConfig>) -> Self {
        Self {
            socket_path,
            listener: None,
            tasks: Arc::new(tasks),
        }
    }

    /// Start the server and listen for connections
    pub async fn start(&mut self) -> Result<()> {
        // Remove socket if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                Error::file_system(self.socket_path.clone(), "remove existing socket", e)
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::file_system(parent.to_path_buf(), "create socket parent directory", e)
            })?;
        }

        // Start Unix domain socket listener
        let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
            Error::configuration(format!(
                "Failed to bind to socket {}: {}",
                self.socket_path.display(),
                e
            ))
        })?;

        self.listener = Some(listener);
        tracing::info!(
            socket_path = %self.socket_path.display(),
            "Task server provider started"
        );

        // Accept connections
        self.handle_connections().await
    }

    /// Handle incoming client connections
    async fn handle_connections(&mut self) -> Result<()> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| Error::configuration("Task server provider not started".to_string()))?;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let tasks = Arc::clone(&self.tasks);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, tasks).await {
                            tracing::error!(error = %e, "Client connection error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to accept connection");
                    // Continue accepting other connections
                }
            }
        }
    }

    /// Handle a single client connection
    async fn handle_client(
        stream: UnixStream,
        tasks: Arc<HashMap<String, TaskConfig>>,
    ) -> Result<()> {
        let (read_half, mut write_half) = stream.into_split();
        let mut buf_reader = BufReader::new(read_half);
        let mut line = String::new();

        while buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read from client: {}", e)))?
            > 0
        {
            // Parse JSON-RPC request
            let request: serde_json::Value = serde_json::from_str(&line.trim())
                .map_err(|e| Error::configuration(format!("Invalid JSON-RPC request: {}", e)))?;

            // Handle the request
            let response = Self::handle_request(request, &tasks).await;

            // Send response
            let response_json = serde_json::to_string(&response).map_err(|e| {
                Error::configuration(format!("Failed to serialize response: {}", e))
            })?;

            write_half
                .write_all(format!("{}\n", response_json).as_bytes())
                .await
                .map_err(|e| Error::configuration(format!("Failed to write response: {}", e)))?;

            line.clear();
        }

        Ok(())
    }

    /// Handle a JSON-RPC request
    async fn handle_request(
        request: serde_json::Value,
        tasks: &HashMap<String, TaskConfig>,
    ) -> serde_json::Value {
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or_default();

        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => {
                // Convert cuenv TaskConfigs to TaskDefinitions
                let task_definitions: Vec<TaskDefinition> = tasks
                    .iter()
                    .map(|(name, config)| TaskDefinition {
                        name: name.clone(),
                        after: config.dependencies.clone().unwrap_or_default(),
                        description: config.description.clone(),
                    })
                    .collect();

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "tasks": task_definitions
                    },
                    "id": id
                })
            }
            "run" => {
                // Extract task name from parameters
                let task_name = request
                    .get("params")
                    .and_then(|p| p.get("task"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default();

                if let Some(task_config) = tasks.get(task_name) {
                    // Execute the task (simplified for now)
                    // In a real implementation, this would use the task executor
                    match Self::execute_task(task_config).await {
                        Ok(exit_code) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "result": {
                                "exit_code": exit_code,
                                "outputs": {}
                            },
                            "id": id
                        }),
                        Err(e) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -1,
                                "message": format!("Task execution failed: {}", e)
                            },
                            "id": id
                        }),
                    }
                } else {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -1,
                            "message": format!("Task not found: {}", task_name)
                        },
                        "id": id
                    })
                }
            }
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                },
                "id": id
            }),
        }
    }

    /// Simple task execution (placeholder implementation)
    async fn execute_task(task_config: &TaskConfig) -> Result<i32> {
        // This is a simplified implementation
        // In practice, this would integrate with the full task executor
        if let Some(command) = &task_config.command {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg(command);

            let output = cmd.output().await.map_err(|e| {
                Error::command_execution(
                    "sh",
                    vec!["-c".to_string(), command.clone()],
                    format!("Failed to execute task: {}", e),
                    None,
                )
            })?;

            Ok(output.status.code().unwrap_or(-1))
        } else {
            // No command specified, consider it successful
            Ok(0)
        }
    }

    /// Export tasks to JSON format for static consumption
    pub fn export_tasks_to_json(&self) -> Result<String> {
        let task_definitions: Vec<TaskDefinition> = self
            .tasks
            .iter()
            .map(|(name, config)| TaskDefinition {
                name: name.clone(),
                after: config.dependencies.clone().unwrap_or_default(),
                description: config.description.clone(),
            })
            .collect();

        let export = serde_json::json!({
            "tasks": task_definitions
        });

        serde_json::to_string_pretty(&export)
            .map_err(|e| Error::configuration(format!("Failed to serialize tasks to JSON: {}", e)))
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(listener) = self.listener.take() {
            drop(listener);
        }

        // Remove socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                Error::file_system(self.socket_path.clone(), "remove socket file", e)
            })?;
        }

        Ok(())
    }
}

impl Drop for TaskServerProvider {
    fn drop(&mut self) {
        // Best effort cleanup
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// Unified task manager that supports both consuming external tasks and providing cuenv tasks
pub struct UnifiedTaskManager {
    /// Manager for consuming external task servers
    pub server_manager: TaskServerManager,
    /// Provider for exposing cuenv tasks (optional)
    pub server_provider: Option<TaskServerProvider>,
    /// Internal task registry
    pub internal_tasks: HashMap<String, TaskConfig>,
}

impl UnifiedTaskManager {
    /// Create a new unified task manager
    pub fn new(socket_dir: PathBuf, internal_tasks: HashMap<String, TaskConfig>) -> Self {
        Self {
            server_manager: TaskServerManager::new(socket_dir),
            server_provider: None,
            internal_tasks,
        }
    }

    /// Start as a task provider server
    pub async fn start_as_provider(&mut self, socket_path: PathBuf) -> Result<()> {
        let mut provider = TaskServerProvider::new(socket_path, self.internal_tasks.clone());
        provider.start().await?;
        self.server_provider = Some(provider);
        Ok(())
    }

    /// Discover and combine both internal and external tasks
    pub async fn discover_all_tasks(
        &mut self,
        discovery_path: Option<&Path>,
    ) -> Result<Vec<TaskDefinition>> {
        let mut all_tasks = Vec::new();

        // Add internal tasks
        for (name, config) in &self.internal_tasks {
            all_tasks.push(TaskDefinition {
                name: format!("cuenv:{}", name),
                after: config.dependencies.clone().unwrap_or_default(),
                description: config.description.clone(),
            });
        }

        // Add external tasks from discovery if path provided
        if let Some(path) = discovery_path {
            let external_tasks = self.server_manager.discover_servers(path).await?;
            all_tasks.extend(external_tasks);
        }

        Ok(all_tasks)
    }

    /// Export internal tasks as JSON
    pub fn export_tasks_to_json(&self) -> Result<String> {
        let task_definitions: Vec<TaskDefinition> = self
            .internal_tasks
            .iter()
            .map(|(name, config)| TaskDefinition {
                name: name.clone(),
                after: config.dependencies.clone().unwrap_or_default(),
                description: config.description.clone(),
            })
            .collect();

        let export = serde_json::json!({
            "tasks": task_definitions
        });

        serde_json::to_string_pretty(&export)
            .map_err(|e| Error::configuration(format!("Failed to serialize tasks to JSON: {}", e)))
    }

    /// Shutdown all components
    pub async fn shutdown(&mut self) -> Result<()> {
        // Shutdown external server manager
        self.server_manager.shutdown().await?;

        // Shutdown task provider if running
        if let Some(provider) = self.server_provider.as_mut() {
            provider.shutdown().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_task_server_provider_creation() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("provider.sock");
        let mut tasks = HashMap::new();
        tasks.insert(
            "test".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Test task".to_string()),
                command: Some("echo hello".to_string()),
                ..Default::default()
            },
        );

        let provider = TaskServerProvider::new(socket_path.clone(), tasks);
        assert_eq!(provider.socket_path, socket_path);
        assert!(provider.listener.is_none());
    }

    #[tokio::test]
    async fn test_unified_task_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "test".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Test task".to_string()),
                ..Default::default()
            },
        );

        let manager = UnifiedTaskManager::new(temp_dir.path().to_path_buf(), tasks.clone());
        assert_eq!(manager.internal_tasks, tasks);
        assert!(manager.server_provider.is_none());
    }

    #[test]
    fn test_export_tasks_to_json() {
        let temp_dir = TempDir::new().unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "build".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Build the project".to_string()),
                dependencies: Some(vec!["deps".to_string()]),
                ..Default::default()
            },
        );

        let manager = UnifiedTaskManager::new(temp_dir.path().to_path_buf(), tasks);
        let json = manager.export_tasks_to_json().unwrap();

        // Verify JSON contains expected structure
        assert!(json.contains("tasks"));
        assert!(json.contains("build"));
        assert!(json.contains("Build the project"));
        assert!(json.contains("deps"));
    }
}
