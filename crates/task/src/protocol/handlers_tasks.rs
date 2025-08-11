//! Task-specific MCP handlers

use super::handlers::parse_env_readonly;

/// Handle list_tasks tool call
pub async fn handle_list_tasks(
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

    match parse_env_readonly(directory, environment, capabilities).await {
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
pub async fn handle_get_task(
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

    match parse_env_readonly(directory, environment, capabilities).await {
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
