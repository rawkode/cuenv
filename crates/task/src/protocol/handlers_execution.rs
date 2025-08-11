//! Execution-specific MCP handlers

use super::handlers::validate_directory;

/// Handle run_task tool call (requires allow_exec)
pub async fn handle_run_task(
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

    let path = match validate_directory(directory) {
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
pub async fn handle_check_directory(
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