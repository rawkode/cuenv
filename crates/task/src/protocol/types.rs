//! JSON-RPC protocol types and structures
//!
//! This module defines the core types used in the Task Server Protocol (TSP)
//! and Model Context Protocol (MCP) communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON-RPC 2.0 request structure
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub method: String,
    pub params: T,
    pub id: u64,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

/// JSON-RPC error structure
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Initialize request parameters (empty)
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeParams {}

/// Initialize response containing available tasks
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    pub tasks: Vec<TaskDefinition>,
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
#[derive(Debug, Serialize, Deserialize)]
pub struct RunTaskParams {
    pub task: String,
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

/// Run task response
#[derive(Debug, Serialize, Deserialize)]
pub struct RunTaskResult {
    pub exit_code: i32,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}
