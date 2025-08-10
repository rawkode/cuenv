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
        let socket_path = self.socket_dir.join(format!("{server_name}.sock"));

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
                    "No task server found for prefix '{prefix}'"
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
    socket_path: Option<PathBuf>,
    listener: Option<UnixListener>,
    tasks: Arc<HashMap<String, TaskConfig>>,
    allow_exec: bool,
    use_stdio: bool,
}

impl TaskServerProvider {
    /// Create a new task server provider for Unix socket
    pub fn new(socket_path: PathBuf, tasks: HashMap<String, TaskConfig>) -> Self {
        Self {
            socket_path: Some(socket_path),
            listener: None,
            tasks: Arc::new(tasks),
            allow_exec: false,
            use_stdio: false,
        }
    }

    /// Create a new task server provider for stdio (MCP mode)
    pub fn new_stdio(tasks: HashMap<String, TaskConfig>, allow_exec: bool) -> Self {
        Self {
            socket_path: None,
            listener: None,
            tasks: Arc::new(tasks),
            allow_exec,
            use_stdio: true,
        }
    }

    /// Create a new task server provider with full options
    pub fn new_with_options(
        socket_path: Option<PathBuf>,
        tasks: HashMap<String, TaskConfig>,
        allow_exec: bool,
        use_stdio: bool,
    ) -> Self {
        Self {
            socket_path,
            listener: None,
            tasks: Arc::new(tasks),
            allow_exec,
            use_stdio,
        }
    }

    /// Start the server and listen for connections
    pub async fn start(&mut self) -> Result<()> {
        if self.use_stdio {
            // Use stdio for MCP mode
            tracing::info!("Task server provider started in stdio mode for MCP");
            self.handle_stdio().await
        } else if let Some(socket_path) = &self.socket_path {
            // Remove socket if it exists
            if socket_path.exists() {
                tokio::fs::remove_file(socket_path).await.map_err(|e| {
                    Error::file_system(socket_path.clone(), "remove existing socket", e)
                })?;
            }

            // Ensure parent directory exists
            if let Some(parent) = socket_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::file_system(parent.to_path_buf(), "create socket parent directory", e)
                })?;
            }

            // Start Unix domain socket listener
            let listener = UnixListener::bind(socket_path).map_err(|e| {
                Error::configuration(format!(
                    "Failed to bind to socket {}: {}",
                    socket_path.display(),
                    e
                ))
            })?;

            self.listener = Some(listener);
            tracing::info!(
                socket_path = %socket_path.display(),
                "Task server provider started"
            );

            // Accept connections
            self.handle_connections().await
        } else {
            Err(Error::configuration(
                "No transport configured: need either socket_path or use_stdio".to_string(),
            ))
        }
    }

    /// Handle stdio communication for MCP mode
    async fn handle_stdio(&mut self) -> Result<()> {
        use tokio::io::{stdin, stdout};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let stdin = stdin();
        let mut stdout = stdout();
        let mut buf_reader = BufReader::new(stdin);
        let mut line = String::new();

        while buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read from stdin: {e}")))?
            > 0
        {
            // Parse JSON-RPC request
            let request: serde_json::Value = serde_json::from_str(line.trim())
                .map_err(|e| Error::configuration(format!("Invalid JSON-RPC request: {e}")))?;

            // Handle the request
            let response = Self::handle_request(request, &self.tasks, self.allow_exec).await;

            // Send response
            let response_json = serde_json::to_string(&response)
                .map_err(|e| Error::configuration(format!("Failed to serialize response: {e}")))?;

            stdout
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .map_err(|e| Error::configuration(format!("Failed to write response: {e}")))?;

            stdout
                .flush()
                .await
                .map_err(|e| Error::configuration(format!("Failed to flush stdout: {e}")))?;

            line.clear();
        }

        Ok(())
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
                    let allow_exec = self.allow_exec;
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, tasks, allow_exec).await {
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
        allow_exec: bool,
    ) -> Result<()> {
        let (read_half, mut write_half) = stream.into_split();
        let mut buf_reader = BufReader::new(read_half);
        let mut line = String::new();

        while buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read from client: {e}")))?
            > 0
        {
            // Parse JSON-RPC request
            let request: serde_json::Value = serde_json::from_str(line.trim())
                .map_err(|e| Error::configuration(format!("Invalid JSON-RPC request: {e}")))?;

            // Handle the request
            let response = Self::handle_request(request, &tasks, allow_exec).await;

            // Send response
            let response_json = serde_json::to_string(&response)
                .map_err(|e| Error::configuration(format!("Failed to serialize response: {e}")))?;

            write_half
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .map_err(|e| Error::configuration(format!("Failed to write response: {e}")))?;

            line.clear();
        }

        Ok(())
    }

    /// Handle a JSON-RPC request (supports both TSP and MCP methods)
    pub async fn handle_request(
        request: serde_json::Value,
        tasks: &HashMap<String, TaskConfig>,
        allow_exec: bool,
    ) -> serde_json::Value {
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or_default();

        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let params = request
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            // TSP Methods (devenv compatibility)
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

            // MCP Methods (Claude Code integration)
            "tools/list" => {
                // List available MCP tools
                let all_tools = Self::get_mcp_tools(allow_exec);

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "tools": all_tools
                    },
                    "id": id
                })
            }
            "tools/call" => Self::handle_mcp_tool_call(params, tasks, allow_exec, id).await,

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

    /// Handle MCP tool call requests
    async fn handle_mcp_tool_call(
        params: serde_json::Value,
        _tasks: &HashMap<String, TaskConfig>,
        allow_exec: bool,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let tool_name = params
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default();

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        match tool_name {
            "cuenv.list_env_vars" => Self::handle_list_env_vars(arguments, id).await,
            "cuenv.get_env_var" => Self::handle_get_env_var(arguments, id).await,
            "cuenv.list_tasks" => Self::handle_list_tasks(arguments, id).await,
            "cuenv.get_task" => Self::handle_get_task(arguments, id).await,
            "cuenv.run_task" => {
                if allow_exec {
                    Self::handle_run_task(arguments, id).await
                } else {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -1,
                            "message": "Task execution not allowed. Start MCP server with --allow-exec flag."
                        },
                        "id": id
                    })
                }
            }
            "cuenv.check_directory" => Self::handle_check_directory(arguments, id).await,
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32601,
                    "message": format!("Tool not found: {}", tool_name)
                },
                "id": id
            }),
        }
    }

    /// Returns the MCP tool definitions
    fn get_mcp_tools(allow_exec: bool) -> Vec<serde_json::Value> {
        let mut tools = vec![
            serde_json::json!({
                "name": "cuenv.list_env_vars",
                "description": "List all environment variables from env.cue configuration",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory containing env.cue file"
                        },
                        "environment": {
                            "type": "string",
                            "description": "Optional environment name (dev, staging, production, etc.)"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional capabilities to enable"
                        }
                    },
                    "required": ["directory"]
                }
            }),
            serde_json::json!({
                "name": "cuenv.get_env_var",
                "description": "Get value of a specific environment variable",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory containing env.cue file"
                        },
                        "name": {
                            "type": "string",
                            "description": "Environment variable name to retrieve"
                        },
                        "environment": {
                            "type": "string",
                            "description": "Optional environment name"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional capabilities to enable"
                        }
                    },
                    "required": ["directory", "name"]
                }
            }),
            serde_json::json!({
                "name": "cuenv.list_tasks",
                "description": "List all available tasks from env.cue configuration",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory containing env.cue file"
                        },
                        "environment": {
                            "type": "string",
                            "description": "Optional environment name"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional capabilities to enable"
                        }
                    },
                    "required": ["directory"]
                }
            }),
            serde_json::json!({
                "name": "cuenv.get_task",
                "description": "Get details for a specific task",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory containing env.cue file"
                        },
                        "name": {
                            "type": "string",
                            "description": "Task name to retrieve"
                        },
                        "environment": {
                            "type": "string",
                            "description": "Optional environment name"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional capabilities to enable"
                        }
                    },
                    "required": ["directory", "name"]
                }
            }),
            serde_json::json!({
                "name": "cuenv.check_directory",
                "description": "Validate if directory has env.cue and is allowed",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory path to check"
                        }
                    },
                    "required": ["directory"]
                }
            }),
        ];

        // Add run_task tool only if execution is allowed
        if allow_exec {
            tools.push(serde_json::json!({
                "name": "cuenv.run_task",
                "description": "Execute a task (requires --allow-exec flag)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory containing env.cue file"
                        },
                        "name": {
                            "type": "string",
                            "description": "Task name to execute"
                        },
                        "args": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Arguments to pass to the task"
                        },
                        "environment": {
                            "type": "string",
                            "description": "Optional environment name"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional capabilities to enable"
                        }
                    },
                    "required": ["directory", "name"]
                }
            }));
        }

        tools
    }

    /// Validate directory and check if it's allowed
    fn validate_directory(directory: &str) -> Result<std::path::PathBuf> {
        let path = std::path::PathBuf::from(directory);

        // Canonicalize the path to resolve symlinks and remove '..' components
        let canonical = path.canonicalize().map_err(|e| {
            Error::configuration(format!(
                "Failed to canonicalize directory '{directory}': {e}"
            ))
        })?;

        // Basic path traversal protection: ensure the path is absolute
        if !canonical.is_absolute() {
            return Err(Error::configuration(format!(
                "Directory path is not absolute after canonicalization: {}",
                canonical.display()
            )));
        }

        // SECURITY: Directory permission validation is done at CLI level.
        // For MCP/TSP mode, we now canonicalize the path and require it to be absolute.
        // If exposing this server to untrusted clients, consider restricting allowed directories further.

        // Check if directory exists (after canonicalization)
        if !canonical.exists() {
            return Err(Error::configuration(format!(
                "Directory does not exist: {}",
                canonical.display()
            )));
        }

        Ok(canonical)
    }

    /// Parses environment configuration in the specified directory without side effects.
    ///
    /// # Parameters
    /// - `directory`: The path to the directory containing the environment configuration to parse.
    /// - `environment`: An optional environment name to select a specific environment configuration.
    /// - `capabilities`: An optional list of capabilities to use during parsing.
    ///
    /// # Returns
    /// Returns a `Result` containing the parsed environment configuration as a `cuenv_config::ParseResult`
    /// on success, or an error if parsing fails.
    async fn parse_env_readonly(
        directory: &str,
        environment: Option<String>,
        capabilities: Option<Vec<String>>,
    ) -> Result<cuenv_config::ParseResult> {
        use cuenv_config::{CueParser, ParseOptions};

        let path = Self::validate_directory(directory)?;

        let options = ParseOptions {
            environment,
            capabilities: capabilities.unwrap_or_default(),
        };

        CueParser::eval_package_with_options(&path, "env", &options)
    }

    /// Handle list_env_vars tool call
    async fn handle_list_env_vars(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        let environment = arguments
            .get("environment")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());
        let capabilities = arguments
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        match Self::parse_env_readonly(directory, environment, capabilities).await {
            Ok(parse_result) => serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&parse_result.variables).unwrap_or_default()
                    }]
                },
                "id": id
            }),
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -1,
                    "message": format!("Failed to list environment variables: {}", e)
                },
                "id": id
            }),
        }
    }

    /// Handle get_env_var tool call
    async fn handle_get_env_var(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        let var_name = arguments
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default();
        let environment = arguments
            .get("environment")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());
        let capabilities = arguments
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        match Self::parse_env_readonly(directory, environment, capabilities).await {
            Ok(parse_result) => {
                let value = parse_result.variables.get(var_name);
                let result = match value {
                    Some(v) => format!("{var_name}={v}"),
                    None => format!("{var_name} not found"),
                };

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": result
                        }]
                    },
                    "id": id
                })
            }
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -1,
                    "message": format!("Failed to get environment variable: {}", e)
                },
                "id": id
            }),
        }
    }

    /// Handle list_tasks tool call
    async fn handle_list_tasks(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        let environment = arguments
            .get("environment")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());
        let capabilities = arguments
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        match Self::parse_env_readonly(directory, environment, capabilities).await {
            Ok(parse_result) => {
                let tasks: Vec<serde_json::Value> = parse_result
                    .tasks
                    .into_iter()
                    .map(|(name, config)| {
                        serde_json::json!({
                            "name": name,
                            "description": config.description.unwrap_or_default(),
                            "dependencies": config.dependencies.unwrap_or_default(),
                            "command": config.command.unwrap_or_default()
                        })
                    })
                    .collect();

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&tasks).unwrap_or_default()
                        }]
                    },
                    "id": id
                })
            }
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -1,
                    "message": format!("Failed to list tasks: {}", e)
                },
                "id": id
            }),
        }
    }

    /// Handle get_task tool call
    async fn handle_get_task(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        let task_name = arguments
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default();
        let environment = arguments
            .get("environment")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());
        let capabilities = arguments
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        match Self::parse_env_readonly(directory, environment, capabilities).await {
            Ok(parse_result) => {
                if let Some(config) = parse_result.tasks.get(task_name) {
                    let task_info = serde_json::json!({
                        "name": task_name,
                        "description": config.description.clone().unwrap_or_default(),
                        "dependencies": config.dependencies.clone().unwrap_or_default(),
                        "command": config.command.clone().unwrap_or_default()
                    });

                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&task_info).unwrap_or_default()
                            }]
                        },
                        "id": id
                    })
                } else {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": format!("Task '{}' not found", task_name)
                            }]
                        },
                        "id": id
                    })
                }
            }
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -1,
                    "message": format!("Failed to get task: {}", e)
                },
                "id": id
            }),
        }
    }

    /// Handle run_task tool call (requires allow_exec)
    async fn handle_run_task(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        let task_name = arguments
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default();
        let task_args = arguments
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let environment = arguments
            .get("environment")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());
        let capabilities = arguments
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        let path = match Self::validate_directory(directory) {
            Ok(p) => p,
            Err(e) => {
                return serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -1,
                        "message": format!("Directory validation failed: {}", e)
                    },
                    "id": id
                })
            }
        };

        // Load environment and create task executor
        use crate::TaskExecutor;
        use cuenv_env::EnvManager;

        let mut env_manager = EnvManager::new();
        match env_manager
            .load_env_with_options(&path, environment, capabilities.unwrap_or_default(), None)
            .await
        {
            Ok(()) => {
                // Create task executor and run the task
                match TaskExecutor::new(env_manager, path).await {
                    Ok(executor) => match executor.execute_task(task_name, &task_args).await {
                        Ok(exit_code) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "result": {
                                "content": [{
                                    "type": "text",
                                    "text": format!("Task '{}' completed with exit code: {}", task_name, exit_code)
                                }]
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
                    },
                    Err(e) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -1,
                            "message": format!("Failed to create task executor: {}", e)
                        },
                        "id": id
                    }),
                }
            }
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -1,
                    "message": format!("Failed to load environment: {}", e)
                },
                "id": id
            }),
        }
    }

    /// Handle check_directory tool call
    async fn handle_check_directory(
        arguments: serde_json::Value,
        id: serde_json::Value,
    ) -> serde_json::Value {
        let directory = arguments
            .get("directory")
            .and_then(|d| d.as_str())
            .unwrap_or_default();

        let path = std::path::PathBuf::from(directory);
        let env_cue = path.join("env.cue");

        let allowed = if path.exists() {
            // For now, assume directories are allowed in MCP mode
            // Directory validation is typically done at the CLI level
            true
        } else {
            false
        };

        let result = serde_json::json!({
            "allowed": allowed,
            "has_env_cue": env_cue.exists(),
            "directory": directory
        });

        serde_json::json!({
            "jsonrpc": "2.0",
            "result": {
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                }]
            },
            "id": id
        })
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
                    format!("Failed to execute task: {e}"),
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
            .map_err(|e| Error::configuration(format!("Failed to serialize tasks to JSON: {e}")))
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(listener) = self.listener.take() {
            drop(listener);
        }

        // Remove socket file
        if let Some(socket_path) = &self.socket_path {
            if socket_path.exists() {
                tokio::fs::remove_file(socket_path).await.map_err(|e| {
                    Error::file_system(socket_path.clone(), "remove socket file", e)
                })?;
            }
        }

        Ok(())
    }
}

impl Drop for TaskServerProvider {
    fn drop(&mut self) {
        // Best effort cleanup - synchronous only in Drop
        if let Some(socket_path) = &self.socket_path {
            if socket_path.exists() {
                let _ = std::fs::remove_file(socket_path);
            }
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
                name: format!("cuenv:{name}"),
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
            .map_err(|e| Error::configuration(format!("Failed to serialize tasks to JSON: {e}")))
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
        assert_eq!(provider.socket_path, Some(socket_path));
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
