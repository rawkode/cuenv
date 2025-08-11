//! Task server provider request handlers (part 2)

use super::handlers::handle_mcp_tool_call;
use super::mcp::{execute_task, get_mcp_tools};
use super::provider::TaskServerProvider;
use super::types::TaskDefinition;
use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use std::collections::HashMap;

impl TaskServerProvider {
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
                    match execute_task(task_config).await {
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
                let all_tools = get_mcp_tools(allow_exec);

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "tools": all_tools
                    },
                    "id": id
                })
            }
            "tools/call" => handle_mcp_tool_call(params, tasks, allow_exec, id).await,

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

    /// Export tasks to JSON format for static consumption
    pub fn export_tasks_to_json(&self) -> Result<String> {
        let task_definitions: Vec<TaskDefinition> = self
            .config
            .get_tasks()
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
