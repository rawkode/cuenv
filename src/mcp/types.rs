use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration options for the MCP server
#[derive(Debug, Clone)]
pub struct McpServerOptions {
    pub transport: String,
    pub port: u16,
    pub allow_exec: bool,
}

/// Parameters for environment variable tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvVarParams {
    /// Directory containing env.cue file
    pub directory: String,
    /// Optional environment name (dev, staging, production, etc.)
    pub environment: Option<String>,
    /// Optional capabilities to enable
    pub capabilities: Option<Vec<String>>,
}

/// Parameters for getting a specific environment variable
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetEnvVarParams {
    /// Directory containing env.cue file
    pub directory: String,
    /// Environment variable name to retrieve
    pub name: String,
    /// Optional environment name (dev, staging, production, etc.)
    pub environment: Option<String>,
    /// Optional capabilities to enable
    pub capabilities: Option<Vec<String>>,
}

/// Parameters for task-related tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskParams {
    /// Directory containing env.cue file
    pub directory: String,
    /// Optional environment name
    pub environment: Option<String>,
    /// Optional capabilities to enable
    pub capabilities: Option<Vec<String>>,
}

/// Parameters for getting a specific task
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskParams {
    /// Directory containing env.cue file
    pub directory: String,
    /// Task name to retrieve
    pub name: String,
    /// Optional environment name
    pub environment: Option<String>,
    /// Optional capabilities to enable
    pub capabilities: Option<Vec<String>>,
}

/// Parameters for running a task
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunTaskParams {
    /// Directory containing env.cue file
    pub directory: String,
    /// Task name to execute
    pub name: String,
    /// Arguments to pass to the task
    pub args: Option<Vec<String>>,
    /// Optional environment name
    pub environment: Option<String>,
    /// Optional capabilities to enable
    pub capabilities: Option<Vec<String>>,
}

/// Parameters for directory validation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DirectoryParams {
    /// Directory path to check
    pub directory: String,
}

/// Response for environment variable listing
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvVarsResponse {
    pub variables: HashMap<String, String>,
}

/// Response for task listing
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TasksResponse {
    pub tasks: Vec<TaskInfo>,
}

/// Information about a single task
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskInfo {
    pub name: String,
    pub description: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub command: Option<String>,
    pub script: Option<String>,
}

/// Response for task execution
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskExecutionResponse {
    pub exit_code: i32,
    pub success: bool,
}

/// Response for directory validation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DirectoryResponse {
    pub allowed: bool,
    pub has_env_cue: bool,
}

/// Response for capabilities listing
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitiesResponse {
    pub capabilities: Vec<String>,
}
