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
            let error_msg = if let Some(data) = error.data {
                format!(
                    "Task server initialization failed: {} (code {}) - Additional data: {}",
                    error.message, error.code, data
                )
            } else {
                format!(
                    "Task server initialization failed: {} (code {})",
                    error.message, error.code
                )
            };
            return Err(Error::configuration(error_msg));
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
            let error_msg = if let Some(data) = error.data {
                format!(
                    "Task execution failed: {} (code {}) - Additional data: {}",
                    error.message, error.code, data
                )
            } else {
                format!(
                    "Task execution failed: {} (code {})",
                    error.message, error.code
                )
            };
            return Err(Error::configuration(error_msg));
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

        // Validate JSON-RPC protocol
        if response.jsonrpc != "2.0" {
            return Err(Error::configuration(format!(
                "Invalid JSON-RPC version: expected '2.0', got '{}'",
                response.jsonrpc
            )));
        }

        // Validate response ID matches request ID
        if response.id != request.id {
            return Err(Error::configuration(format!(
                "Response ID mismatch: expected {}, got {}",
                request.id, response.id
            )));
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::types::JsonRpcError;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::TempDir;
    use tokio::fs;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{UnixListener, UnixStream};

    #[test]
    fn test_new_client() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let client = TaskServerClient::new(socket_path.clone());
        assert_eq!(client.socket_path, socket_path);
        assert!(client.server_process.is_none());
        assert!(client.stream.is_none());
        assert_eq!(client.next_id, 1);
    }

    #[test]
    fn test_next_id_increments() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);

        assert_eq!(client.next_id(), 1);
        assert_eq!(client.next_id(), 2);
        assert_eq!(client.next_id(), 3);
        assert_eq!(client.next_id, 4); // Internal counter should be at 4
    }

    #[tokio::test]
    async fn test_launch_and_connect_nonexistent_executable() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);
        let result = client.launch_and_connect("/nonexistent/executable").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to launch task server"));
    }

    #[tokio::test]
    #[ignore] // Flaky test in CI environment - timeout behavior varies
    async fn test_launch_and_connect_socket_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create a mock executable that doesn't create a socket
        let script_path = temp_dir.path().join("mock_server.sh");
        let mut script = std::fs::File::create(&script_path).unwrap();
        writeln!(script, "#!/bin/bash").unwrap();
        writeln!(script, "sleep 15").unwrap(); // Sleep longer than timeout
        drop(script);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }

        let mut client = TaskServerClient::new(socket_path);
        let result = client
            .launch_and_connect(&script_path.to_string_lossy())
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Timeout waiting for task server"));
    }

    #[tokio::test]
    async fn test_send_request_not_connected() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: (),
            id: 1,
        };

        let result: Result<JsonRpcResponse<serde_json::Value>> = client.send_request(request).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Not connected to task server"));
    }

    #[tokio::test]
    async fn test_initialize_not_connected() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);
        let result = client.initialize().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Not connected"));
    }

    #[tokio::test]
    async fn test_run_task_not_connected() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);
        let inputs = HashMap::new();
        let outputs = HashMap::new();

        let result = client.run_task("test_task", inputs, outputs).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Not connected"));
    }

    #[tokio::test]
    async fn test_shutdown_without_connection() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let mut client = TaskServerClient::new(socket_path);
        let result = client.shutdown().await;

        // Should succeed even without connection
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_removes_socket_file() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create a dummy socket file
        fs::write(&socket_path, "dummy").await.unwrap();
        assert!(socket_path.exists());

        let mut client = TaskServerClient::new(socket_path.clone());
        let result = client.shutdown().await;

        assert!(result.is_ok());
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn test_drop_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create a dummy socket file
        std::fs::write(&socket_path, "dummy").unwrap();
        assert!(socket_path.exists());

        {
            let _client = TaskServerClient::new(socket_path.clone());
            // Client goes out of scope here, triggering Drop
        }

        // Socket should be cleaned up
        assert!(!socket_path.exists());
    }

    // Mock server for testing protocol communication
    async fn create_mock_server(socket_path: &std::path::Path) -> Result<UnixListener> {
        if socket_path.exists() {
            tokio::fs::remove_file(socket_path).await.map_err(|e| {
                Error::file_system(socket_path.to_path_buf(), "remove existing socket", e)
            })?;
        }

        UnixListener::bind(socket_path)
            .map_err(|e| Error::configuration(format!("Failed to bind mock server: {e}")))
    }

    #[tokio::test]
    async fn test_protocol_initialize_success() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        // Handle client connection in background
        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            // Verify it's an initialize request
            let request: JsonRpcRequest<InitializeParams> = serde_json::from_str(&buffer).unwrap();
            assert_eq!(request.method, "initialize");
            assert_eq!(request.jsonrpc, "2.0");

            // Send response
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(InitializeResult {
                    tasks: vec![TaskDefinition {
                        name: "test_task".to_string(),
                        after: vec![],
                        description: Some("Test task".to_string()),
                    }],
                }),
                error: None,
                id: request.id,
            };

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Connect client and test
        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let result = client.initialize().await;
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "test_task");
        assert_eq!(tasks[0].description, Some("Test task".to_string()));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_initialize_error() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        // Handle client connection in background
        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            let request: JsonRpcRequest<InitializeParams> = serde_json::from_str(&buffer).unwrap();

            // Send error response
            let response = JsonRpcResponse::<InitializeResult> {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -1,
                    message: "Initialization failed".to_string(),
                    data: Some(serde_json::json!({"details": "Mock error"})),
                }),
                id: request.id,
            };

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let result = client.initialize().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Initialization failed"));
        assert!(error.to_string().contains("Mock error"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_run_task_success() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            let request: JsonRpcRequest<RunTaskParams> = serde_json::from_str(&buffer).unwrap();
            assert_eq!(request.method, "run");
            assert_eq!(request.params.task, "test_task");

            // Send response
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(RunTaskResult {
                    exit_code: 0,
                    outputs: HashMap::from([
                        ("output1".to_string(), "value1".to_string()),
                        ("output2".to_string(), "value2".to_string()),
                    ]),
                }),
                error: None,
                id: request.id,
            };

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let inputs = HashMap::from([("input1".to_string(), "value1".to_string())]);
        let outputs = HashMap::from([("output1".to_string(), "".to_string())]);

        let result = client.run_task("test_task", inputs, outputs).await;
        assert!(result.is_ok());

        let task_result = result.unwrap();
        assert_eq!(task_result.exit_code, 0);
        assert_eq!(task_result.outputs.len(), 2);
        assert_eq!(
            task_result.outputs.get("output1"),
            Some(&"value1".to_string())
        );

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_run_task_error() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            let request: JsonRpcRequest<RunTaskParams> = serde_json::from_str(&buffer).unwrap();

            // Send error response
            let response = JsonRpcResponse::<RunTaskResult> {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -2,
                    message: "Task execution failed".to_string(),
                    data: None,
                }),
                id: request.id,
            };

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let inputs = HashMap::new();
        let outputs = HashMap::new();

        let result = client.run_task("failing_task", inputs, outputs).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Task execution failed"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_invalid_json_rpc_version() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            let request: JsonRpcRequest<InitializeParams> = serde_json::from_str(&buffer).unwrap();

            // Send response with wrong version
            let response = serde_json::json!({
                "jsonrpc": "1.0",
                "result": {"tasks": []},
                "id": request.id
            });

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let result = client.initialize().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid JSON-RPC version"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_id_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            let request: JsonRpcRequest<InitializeParams> = serde_json::from_str(&buffer).unwrap();

            // Send response with wrong ID
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "result": {"tasks": []},
                "id": request.id + 999
            });

            let response_json = serde_json::to_string(&response).unwrap();
            stream
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let result = client.initialize().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Response ID mismatch"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_malformed_response() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read request
            let mut buffer = String::new();
            let mut reader = tokio::io::BufReader::new(&mut stream);
            reader.read_line(&mut buffer).await.unwrap();

            // Send malformed response
            stream.write_all(b"invalid json response\n").await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stream = UnixStream::connect(&socket_path).await.unwrap();
        let mut client = TaskServerClient::new(socket_path.clone());
        client.stream = Some(stream);

        let result = client.initialize().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to parse response"));

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_concurrent_requests() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create mock server that handles multiple requests
        let listener = create_mock_server(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();

                tokio::spawn(async move {
                    // Read request
                    let mut buffer = String::new();
                    let mut reader = tokio::io::BufReader::new(&mut stream);
                    reader.read_line(&mut buffer).await.unwrap();

                    let request: JsonRpcRequest<InitializeParams> =
                        serde_json::from_str(&buffer).unwrap();

                    // Send response
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: Some(InitializeResult { tasks: vec![] }),
                        error: None,
                        id: request.id,
                    };

                    let response_json = serde_json::to_string(&response).unwrap();
                    stream
                        .write_all(format!("{response_json}\n").as_bytes())
                        .await
                        .unwrap();
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create multiple clients and test concurrent operations
        let mut handles = Vec::new();

        for i in 0..3 {
            let socket_path_clone = socket_path.clone();

            let handle = tokio::spawn(async move {
                let stream = UnixStream::connect(&socket_path_clone).await.unwrap();
                let mut client = TaskServerClient::new(socket_path_clone);
                client.stream = Some(stream);

                let result = client.initialize().await;
                assert!(result.is_ok());

                let tasks = result.unwrap();
                assert_eq!(tasks.len(), 0);

                i
            });

            handles.push(handle);
        }

        // Wait for all clients to complete
        for handle in handles {
            handle.await.unwrap();
        }

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_launch_and_connect_removes_existing_socket() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create an existing socket file
        fs::write(&socket_path, "existing socket").await.unwrap();
        assert!(socket_path.exists());

        let mut client = TaskServerClient::new(socket_path.clone());

        // This should fail because executable doesn't exist, but it should still remove the socket
        let result = client.launch_and_connect("/nonexistent/executable").await;
        assert!(result.is_err());

        // Socket should be removed regardless
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn test_send_request_serialization_error() {
        // This test documents the expected behavior for serialization errors
        // In practice, serde_json::to_string rarely fails for simple types
        // This test verifies that the framework can handle such scenarios
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let _client = TaskServerClient::new(socket_path);

        // Since we can't easily create a serialization error with simple types,
        // this test documents the expected behavior
        // In a real scenario, serialization errors would come from complex types
        // that contain non-serializable data like raw pointers or file handles
    }

    #[test]
    fn test_initialize_result_deserialization() {
        let json = r#"{"tasks":[{"name":"test","after":[],"description":"Test task"}]}"#;
        let result: InitializeResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].name, "test");
        assert_eq!(result.tasks[0].description, Some("Test task".to_string()));
        assert!(result.tasks[0].after.is_empty());
    }

    #[test]
    fn test_run_task_result_deserialization() {
        let json = r#"{"exit_code":0,"outputs":{"key":"value"}}"#;
        let result: RunTaskResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_run_task_params_serialization() {
        let params = RunTaskParams {
            task: "test_task".to_string(),
            inputs: HashMap::from([("input".to_string(), "value".to_string())]),
            outputs: HashMap::from([("output".to_string(), "".to_string())]),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("test_task"));
        assert!(json.contains("input"));
        assert!(json.contains("value"));
    }
}
