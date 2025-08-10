//! Integration tests for MCP (Model Context Protocol) server functionality
//!
//! These tests verify that the MCP server correctly handles tool calls and maintains
//! compatibility with the existing Task Server Protocol (TSP).

use cuenv_config::TaskConfig;
use cuenv_core::Result;
use cuenv_task::TaskServerProvider;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Test that MCP tools/list method returns available tools
#[tokio::test]
async fn test_mcp_tools_list() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_mcp.sock");
    
    // Create some test tasks
    let mut tasks = HashMap::new();
    tasks.insert(
        "test_task".to_string(),
        TaskConfig {
            description: Some("Test task".to_string()),
            command: Some(vec!["echo".to_string(), "test".to_string()]),
            ..Default::default()
        },
    );

    // Create MCP server with execution allowed
    let mut provider = TaskServerProvider::new_stdio(tasks, true);

    // Test tools/list request
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), true).await;
    
    // Verify response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["tools"].is_array());
    
    let tools = response["result"]["tools"].as_array().unwrap();
    
    // Check that MCP tools are present
    let tool_names: Vec<String> = tools.iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
        
    assert!(tool_names.contains(&"cuenv.list_env_vars".to_string()));
    assert!(tool_names.contains(&"cuenv.get_env_var".to_string()));
    assert!(tool_names.contains(&"cuenv.list_tasks".to_string()));
    assert!(tool_names.contains(&"cuenv.get_task".to_string()));
    assert!(tool_names.contains(&"cuenv.check_directory".to_string()));
    assert!(tool_names.contains(&"cuenv.run_task".to_string())); // Should be present with allow_exec=true
    
    Ok(())
}

/// Test that MCP tools/list method excludes run_task when allow_exec is false
#[tokio::test]
async fn test_mcp_tools_list_no_exec() -> Result<()> {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), false).await;
    
    let tools = response["result"]["tools"].as_array().unwrap();
    let tool_names: Vec<String> = tools.iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
        
    assert!(!tool_names.contains(&"cuenv.run_task".to_string())); // Should be absent with allow_exec=false
    
    Ok(())
}

/// Test MCP check_directory tool call
#[tokio::test]
async fn test_mcp_check_directory() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    
    // Create an env.cue file in the temp directory
    let env_cue_path = temp_dir.path().join("env.cue");
    std::fs::write(&env_cue_path, "package main\nenvironment: dev: {}")?;
    
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "cuenv.check_directory",
            "arguments": {
                "directory": temp_dir.path().to_str().unwrap()
            }
        },
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), false).await;
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["content"].is_array());
    
    let content = response["result"]["content"][0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(content).unwrap();
    
    assert_eq!(result["allowed"], true); // In MCP mode, directories are assumed allowed
    assert_eq!(result["has_env_cue"], true);
    
    Ok(())
}

/// Test TSP initialize method still works (backward compatibility)
#[tokio::test]
async fn test_tsp_initialize_compatibility() -> Result<()> {
    let mut tasks = HashMap::new();
    tasks.insert(
        "build".to_string(),
        TaskConfig {
            description: Some("Build the project".to_string()),
            dependencies: Some(vec!["deps".to_string()]),
            ..Default::default()
        },
    );

    let request = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {},
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &tasks, false).await;
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["tasks"].is_array());
    
    let task_list = response["result"]["tasks"].as_array().unwrap();
    assert_eq!(task_list.len(), 1);
    assert_eq!(task_list[0]["name"], "build");
    assert_eq!(task_list[0]["description"], "Build the project");
    assert_eq!(task_list[0]["after"], json!(["deps"]));
    
    Ok(())
}

/// Test MCP tool call with invalid tool name
#[tokio::test]
async fn test_mcp_invalid_tool() -> Result<()> {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "cuenv.invalid_tool",
            "arguments": {}
        },
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), false).await;
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32601);
    assert!(response["error"]["message"].as_str().unwrap().contains("Tool not found"));
    
    Ok(())
}

/// Test that run_task tool call is blocked without allow_exec
#[tokio::test]
async fn test_mcp_run_task_blocked() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "cuenv.run_task",
            "arguments": {
                "directory": temp_dir.path().to_str().unwrap(),
                "name": "test_task"
            }
        },
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), false).await;
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["error"].is_object());
    assert!(response["error"]["message"].as_str().unwrap().contains("not allowed"));
    
    Ok(())
}

/// Test invalid JSON-RPC method
#[tokio::test]
async fn test_invalid_method() -> Result<()> {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "invalid_method",
        "id": 1
    });

    let response = TaskServerProvider::handle_request(request, &HashMap::new(), false).await;
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32601);
    assert!(response["error"]["message"].as_str().unwrap().contains("Method not found"));
    
    Ok(())
}

/// Test MCP server creation with different configurations
#[tokio::test]
async fn test_mcp_server_creation() -> Result<()> {
    let tasks = HashMap::new();
    
    // Test stdio server creation
    let stdio_server = TaskServerProvider::new_stdio(tasks.clone(), true);
    assert!(stdio_server.use_stdio);
    assert!(stdio_server.allow_exec);
    
    // Test Unix socket server creation
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test.sock");
    
    let unix_server = TaskServerProvider::new_with_options(
        Some(socket_path), 
        tasks.clone(), 
        false, 
        false
    );
    assert!(!unix_server.use_stdio);
    assert!(!unix_server.allow_exec);
    
    Ok(())
}