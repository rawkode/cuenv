use crate::cue_parser::{CueParser, ParseOptions};
use crate::directory::DirectoryManager;
use crate::env_manager::EnvManager;
use crate::errors::{Error, Result};
use crate::mcp::types::*;
use crate::task_executor::TaskExecutor;
use rmcp_macros::{tool, tool_box};
use std::collections::HashMap;
use std::path::PathBuf;

/// Tool box containing all cuenv MCP tools
#[tool_box]
pub struct CuenvToolBox {
    pub allow_exec: bool,
}

/// Validate directory and check if it's allowed
fn validate_directory(directory: &str) -> Result<PathBuf> {
    let path = PathBuf::from(directory);

    // Check if directory exists
    if !path.exists() {
        return Err(Error::configuration(format!(
            "Directory does not exist: {}",
            directory
        )));
    }

    // Check if directory is allowed
    let dir_manager = DirectoryManager::new();
    if !dir_manager.is_directory_allowed(&path)? {
        return Err(Error::permission_denied(
            "directory access",
            format!(
                "Directory not allowed: {}. Run 'cuenv allow {}' to allow this directory.",
                directory, directory
            ),
        ));
    }

    Ok(path)
}

/// Parse environment without side effects
fn parse_env_readonly(
    directory: &str,
    environment: Option<String>,
    capabilities: Option<Vec<String>>,
) -> Result<crate::cue_parser::ParseResult> {
    let path = validate_directory(directory)?;

    let options = ParseOptions {
        environment,
        capabilities: capabilities.unwrap_or_default(),
    };

    CueParser::eval_package_with_options(&path, "env", &options)
}

impl CuenvToolBox {
    /// List all environment variables
    #[tool]
    pub async fn list_env_vars(&self, params: EnvVarParams) -> Result<EnvVarsResponse> {
        let parse_result =
            parse_env_readonly(&params.directory, params.environment, params.capabilities)?;

        Ok(EnvVarsResponse {
            variables: parse_result.variables,
        })
    }

    /// Get a specific environment variable
    #[tool]
    pub async fn get_env_var(&self, params: GetEnvVarParams) -> Result<Option<String>> {
        let parse_result =
            parse_env_readonly(&params.directory, params.environment, params.capabilities)?;

        Ok(parse_result.variables.get(&params.name).cloned())
    }

    /// List available environments (dev, staging, production, etc.)
    #[tool]
    pub async fn list_environments(&self, params: DirectoryParams) -> Result<Vec<String>> {
        let path = validate_directory(&params.directory)?;

        // Parse the CUE package to get environment information
        let parse_result =
            CueParser::eval_package_with_options(&path, "env", &ParseOptions::default())?;

        // Extract environment names from the parse result
        // This would need to be implemented based on how environments are structured in CUE
        // For now, return common environment names if env.cue exists
        let env_cue = path.join("env.cue");
        if env_cue.exists() {
            Ok(vec![
                "dev".to_string(),
                "staging".to_string(),
                "production".to_string(),
            ])
        } else {
            Ok(vec![])
        }
    }

    /// List all available tasks
    #[tool]
    pub async fn list_tasks(&self, params: TaskParams) -> Result<TasksResponse> {
        let parse_result =
            parse_env_readonly(&params.directory, params.environment, params.capabilities)?;

        let tasks = parse_result
            .tasks
            .into_iter()
            .map(|(name, config)| TaskInfo {
                name,
                description: config.description,
                dependencies: config.dependencies,
                command: config.command,
                script: config.script,
            })
            .collect();

        Ok(TasksResponse { tasks })
    }

    /// Get details for a specific task
    #[tool]
    pub async fn get_task(&self, params: GetTaskParams) -> Result<Option<TaskInfo>> {
        let parse_result =
            parse_env_readonly(&params.directory, params.environment, params.capabilities)?;

        if let Some(config) = parse_result.tasks.get(&params.name) {
            Ok(Some(TaskInfo {
                name: params.name,
                description: config.description.clone(),
                dependencies: config.dependencies.clone(),
                command: config.command.clone(),
                script: config.script.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Execute a task (requires allow_exec flag)
    #[tool]
    pub async fn run_task(&self, params: RunTaskParams) -> Result<TaskExecutionResponse> {
        if !self.allow_exec {
            return Err(Error::permission_denied(
                "task execution",
                "Task execution not allowed. Start MCP server with --allow-exec flag.",
            ));
        }

        let path = validate_directory(&params.directory)?;

        // Load environment and create task executor
        let mut env_manager = EnvManager::new();
        env_manager
            .load_env_with_options(
                &path,
                params.environment,
                params.capabilities.unwrap_or_default(),
                None,
            )
            .await?;

        let executor = TaskExecutor::new(env_manager, path).await?;

        // Execute the task
        let args = params.args.unwrap_or_default();
        let exit_code = executor.execute_task(&params.name, &args).await?;

        Ok(TaskExecutionResponse {
            exit_code,
            success: exit_code == 0,
        })
    }

    /// Check if a directory is valid and allowed
    #[tool]
    pub async fn check_directory(&self, params: DirectoryParams) -> Result<DirectoryResponse> {
        let path = PathBuf::from(&params.directory);
        let env_cue = path.join("env.cue");

        let allowed = if path.exists() {
            let dir_manager = DirectoryManager::new();
            dir_manager.is_directory_allowed(&path).unwrap_or(false)
        } else {
            false
        };

        Ok(DirectoryResponse {
            allowed,
            has_env_cue: env_cue.exists(),
        })
    }

    /// List available capabilities
    #[tool]
    pub async fn list_capabilities(&self, params: DirectoryParams) -> Result<CapabilitiesResponse> {
        let path = validate_directory(&params.directory)?;

        // Parse the CUE package to extract capability information
        let parse_result =
            CueParser::eval_package_with_options(&path, "env", &ParseOptions::default())?;

        // Extract unique capabilities from variable metadata
        // This would need to be implemented based on how capabilities are stored
        // For now, return empty list as capabilities are not directly exposed in ParseResult
        Ok(CapabilitiesResponse {
            capabilities: vec![],
        })
    }
}
