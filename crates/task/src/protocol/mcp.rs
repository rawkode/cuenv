//! Model Context Protocol (MCP) handlers for Claude Code integration

use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};

/// Returns the MCP tool definitions
pub fn get_mcp_tools(allow_exec: bool) -> Vec<serde_json::Value> {
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

/// Simple task execution (placeholder implementation)
pub async fn execute_task(task_config: &TaskConfig) -> Result<i32> {
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