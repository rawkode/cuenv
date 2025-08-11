//! Task server manager that handles multiple external task servers

use super::client::TaskServerClient;
use super::types::TaskDefinition;
use cuenv_core::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Task server manager that handles multiple external task servers
pub struct TaskServerManager {
    servers: Vec<TaskServerClient>,
    pub(crate) socket_dir: PathBuf,
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