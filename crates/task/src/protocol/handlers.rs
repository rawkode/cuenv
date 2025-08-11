//! MCP tool call handlers

use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use std::collections::HashMap;

/// Validate directory and check if it's allowed
pub fn validate_directory(directory: &str) -> Result<std::path::PathBuf> {
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
pub async fn parse_env_readonly(
    directory: &str,
    environment: Option<String>,
    capabilities: Option<Vec<String>>,
) -> Result<cuenv_config::ParseResult> {
    use cuenv_config::{CueParser, ParseOptions};

    let path = validate_directory(directory)?;

    let options = ParseOptions {
        environment,
        capabilities: capabilities.unwrap_or_default(),
    };

    CueParser::eval_package_with_options(&path, "env", &options)
}

/// Handle MCP tool call requests
pub async fn handle_mcp_tool_call(
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
        "cuenv.list_env_vars" => handle_list_env_vars(arguments, id).await,
        "cuenv.get_env_var" => handle_get_env_var(arguments, id).await,
        "cuenv.list_tasks" => super::handlers_tasks::handle_list_tasks(arguments, id).await,
        "cuenv.get_task" => super::handlers_tasks::handle_get_task(arguments, id).await,
        "cuenv.run_task" => {
            if allow_exec {
                super::handlers_execution::handle_run_task(arguments, id).await
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
        "cuenv.check_directory" => super::handlers_execution::handle_check_directory(arguments, id).await,
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

/// Handle list_env_vars tool call
pub async fn handle_list_env_vars(
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
pub async fn handle_get_env_var(
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

    match parse_env_readonly(directory, environment, capabilities).await {
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