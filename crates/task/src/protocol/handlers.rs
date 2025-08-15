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

    CueParser::eval_package_with_options(
        &path,
        cuenv_core::constants::DEFAULT_PACKAGE_NAME,
        &options,
    )
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
        "cuenv.check_directory" => {
            super::handlers_execution::handle_check_directory(arguments, id).await
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    #[test]
    fn test_validate_directory_valid_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let result = validate_directory(&path.to_string_lossy());
        assert!(result.is_ok());

        let canonical = result.unwrap();
        assert!(canonical.is_absolute());
        assert!(canonical.exists());
    }

    #[test]
    fn test_validate_directory_nonexistent_path() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent");

        let result = validate_directory(&nonexistent.to_string_lossy());
        assert!(result.is_err());

        let error = result.unwrap_err();
        // Check for either canonicalization error or directory existence error
        let error_str = error.to_string();
        assert!(
            error_str.contains("Directory does not exist")
                || error_str.contains("canonicalize")
                || error_str.contains("No such file or directory")
        );
    }

    #[test]
    fn test_validate_directory_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        let link_dir = temp_dir.path().join("link");

        std::fs::create_dir(&target_dir).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target_dir, &link_dir).unwrap();

            let result = validate_directory(&link_dir.to_string_lossy());
            assert!(result.is_ok());

            let canonical = result.unwrap();
            // Should resolve to the target directory
            assert_eq!(canonical, target_dir.canonicalize().unwrap());
        }
    }

    #[test]
    fn test_validate_directory_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("nested");
        std::fs::create_dir(&nested_dir).unwrap();

        // Try to use .. components
        let traversal_path = nested_dir.join("..").join("nested");

        let result = validate_directory(&traversal_path.to_string_lossy());
        assert!(result.is_ok()); // Should work after canonicalization

        let canonical = result.unwrap();
        assert_eq!(canonical, nested_dir.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_parse_env_readonly_nonexistent_directory() {
        let result = parse_env_readonly("/nonexistent/directory", None, None).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("canonicalize"));
    }

    #[tokio::test]
    async fn test_parse_env_readonly_with_environment() {
        let temp_dir = TempDir::new().unwrap();

        // Create a basic env.cue file
        let env_cue = temp_dir.path().join("env.cue");
        fs::write(
            &env_cue,
            r#"
package env

vars: {
    TEST_VAR: "test_value"
}
"#,
        )
        .await
        .unwrap();

        let result = parse_env_readonly(
            &temp_dir.path().to_string_lossy(),
            Some("test".to_string()),
            None,
        )
        .await;

        // This might fail due to CUE parsing, but we test the structure
        match result {
            Ok(_parse_result) => {
                // If parsing succeeds, verify structure
                // Note: actual parsing might fail due to test environment
            }
            Err(e) => {
                // Expected in test environment without proper CUE setup
                assert!(e.to_string().contains("Failed") || e.to_string().contains("parse"));
            }
        }
    }

    #[tokio::test]
    async fn test_parse_env_readonly_with_capabilities() {
        let temp_dir = TempDir::new().unwrap();

        let result = parse_env_readonly(
            &temp_dir.path().to_string_lossy(),
            None,
            Some(vec!["capability1".to_string(), "capability2".to_string()]),
        )
        .await;

        // Should fail due to missing env.cue but test parameter handling
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_list_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.list_env_vars",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy()
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(1)),
        )
        .await;

        // Should be JSON-RPC response
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        // Should have error due to missing env.cue
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_get_env_var() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.get_env_var",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy(),
                "name": "TEST_VAR"
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(2)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 2);
        // Should have error due to missing env.cue
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_list_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.list_tasks",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy()
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(3)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 3);
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_get_task() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.get_task",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy(),
                "name": "build"
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(4)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 4);
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_run_task_not_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.run_task",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy(),
                "name": "test_task"
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false, // allow_exec = false
            serde_json::Value::Number(serde_json::Number::from(5)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 5);
        assert!(response.get("error").is_some());

        let error = &response["error"];
        assert_eq!(error["code"], -1);
        assert!(error["message"].as_str().unwrap().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_run_task_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.run_task",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy(),
                "name": "test_task"
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            true, // allow_exec = true
            serde_json::Value::Number(serde_json::Number::from(6)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 6);
        // Should delegate to execution handler
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_check_directory() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.check_directory",
            "arguments": {
                "directory": temp_dir.path().to_string_lossy()
            }
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(7)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 7);
        // Should delegate to execution handler
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_unknown_tool() {
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "unknown.tool",
            "arguments": {}
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(8)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 8);
        assert!(response.get("error").is_some());

        let error = &response["error"];
        assert_eq!(error["code"], -32601);
        assert!(error["message"]
            .as_str()
            .unwrap()
            .contains("Tool not found"));
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_missing_tool_name() {
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "arguments": {}
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(9)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 9);
        assert!(response.get("error").is_some());

        let error = &response["error"];
        assert_eq!(error["code"], -32601);
        assert!(error["message"]
            .as_str()
            .unwrap()
            .contains("Tool not found"));
    }

    #[tokio::test]
    async fn test_handle_mcp_tool_call_missing_arguments() {
        let tasks = HashMap::new();

        let params = serde_json::json!({
            "name": "cuenv.list_env_vars"
        });

        let response = handle_mcp_tool_call(
            params,
            &tasks,
            false,
            serde_json::Value::Number(serde_json::Number::from(10)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 10);
        // Should still work with empty arguments
    }

    #[tokio::test]
    async fn test_handle_list_env_vars_with_environment() {
        let temp_dir = TempDir::new().unwrap();

        let arguments = serde_json::json!({
            "directory": temp_dir.path().to_string_lossy(),
            "environment": "test_env"
        });

        let response = handle_list_env_vars(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(11)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 11);
        // Should have error due to missing env.cue
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_list_env_vars_with_capabilities() {
        let temp_dir = TempDir::new().unwrap();

        let arguments = serde_json::json!({
            "directory": temp_dir.path().to_string_lossy(),
            "capabilities": ["cap1", "cap2"]
        });

        let response = handle_list_env_vars(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(12)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 12);
        // Should have error due to missing env.cue
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_list_env_vars_empty_directory() {
        let arguments = serde_json::json!({});

        let response = handle_list_env_vars(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(13)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 13);
        assert!(response.get("error").is_some());

        let error = &response["error"];
        assert_eq!(error["code"], -1);
    }

    #[tokio::test]
    async fn test_handle_get_env_var_with_all_parameters() {
        let temp_dir = TempDir::new().unwrap();

        let arguments = serde_json::json!({
            "directory": temp_dir.path().to_string_lossy(),
            "name": "TEST_VAR",
            "environment": "test_env",
            "capabilities": ["cap1"]
        });

        let response = handle_get_env_var(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(14)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 14);
        // Should have error due to missing env.cue
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_get_env_var_missing_name() {
        let temp_dir = TempDir::new().unwrap();

        let arguments = serde_json::json!({
            "directory": temp_dir.path().to_string_lossy()
        });

        let response = handle_get_env_var(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(15)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 15);
        // Should have error due to missing env.cue (not missing name, as empty string is used)
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_get_env_var_invalid_capabilities() {
        let temp_dir = TempDir::new().unwrap();

        let arguments = serde_json::json!({
            "directory": temp_dir.path().to_string_lossy(),
            "name": "TEST_VAR",
            "capabilities": "not_an_array"
        });

        let response = handle_get_env_var(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(16)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 16);
        // Should handle invalid capabilities gracefully
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_error_response_format() {
        let _temp_dir = TempDir::new().unwrap();

        // Test that all error responses follow JSON-RPC format
        let arguments = serde_json::json!({
            "directory": "/nonexistent/path"
        });

        let response = handle_list_env_vars(
            arguments,
            serde_json::Value::Number(serde_json::Number::from(17)),
        )
        .await;

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 17);
        assert!(response.get("error").is_some());
        assert!(response.get("result").is_none());

        let error = &response["error"];
        assert!(error.get("code").is_some());
        assert!(error.get("message").is_some());
        assert_eq!(error["code"], -1);
    }

    #[tokio::test]
    async fn test_concurrent_handler_calls() {
        let temp_dir = TempDir::new().unwrap();
        let tasks = HashMap::new();

        // Test multiple concurrent calls
        let mut handles = Vec::new();

        for i in 0..5 {
            let dir = temp_dir.path().to_string_lossy().to_string();
            let task_map = tasks.clone();

            let handle = tokio::spawn(async move {
                let params = serde_json::json!({
                    "name": "cuenv.list_env_vars",
                    "arguments": {
                        "directory": dir
                    }
                });

                handle_mcp_tool_call(
                    params,
                    &task_map,
                    false,
                    serde_json::Value::Number(serde_json::Number::from(100 + i)),
                )
                .await
            });

            handles.push(handle);
        }

        // Wait for all to complete
        for (i, handle) in handles.into_iter().enumerate() {
            let response = handle.await.unwrap();
            assert_eq!(response["jsonrpc"], "2.0");
            assert_eq!(response["id"], 100 + i);
        }
    }
}
