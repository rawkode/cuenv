use crate::cue_parser::{CueParser, ParseOptions};
use crate::directory::DirectoryManager;
use crate::env_manager::EnvManager;
use crate::errors::{Error, Result};
use crate::mcp::types::*;
use crate::task_executor::TaskExecutor;
use rmcp_macros::tool;
use std::future::Future;
use std::path::PathBuf;

/// Tool box containing all cuenv MCP tools
#[derive(Clone)]
pub struct CuenvToolBox {
    pub allow_exec: bool,
}

/// Validate directory and check if it's allowed
fn validate_directory(directory: &str) -> Result<PathBuf> {
    // Reject empty directory paths
    if directory.is_empty() {
        return Err(Error::configuration("Directory path cannot be empty"));
    }

    // Reject paths with null bytes
    if directory.contains('\0') {
        return Err(Error::configuration(
            "Directory path contains invalid characters",
        ));
    }

    let path = PathBuf::from(directory);

    // Canonicalize path to prevent path traversal attacks
    // This resolves symlinks, removes . and .. components, and returns an absolute path
    let canonical_path = path.canonicalize().map_err(|e| {
        Error::configuration(format!(
            "Cannot canonicalize directory path '{directory}': {e}. Directory may not exist or be accessible."
        ))
    })?;

    // Ensure the canonical path is still a directory
    if !canonical_path.is_dir() {
        return Err(Error::configuration(format!(
            "Path is not a directory: {}",
            canonical_path.display()
        )));
    }

    // Check if directory is allowed
    let dir_manager = DirectoryManager::new();
    if !dir_manager.is_directory_allowed(&canonical_path)? {
        return Err(Error::permission_denied(
            "directory access",
            format!(
                "Directory not allowed: {}. Run 'cuenv allow {}' to allow this directory.",
                canonical_path.display(),
                canonical_path.display()
            ),
        ));
    }

    Ok(canonical_path)
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
        let parse_result = parse_env_readonly(
            &params.directory,
            params.environment.clone(),
            params.capabilities,
        )?;

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
        let env_cue = path.join("env.cue");

        // If no env.cue file exists, return empty list
        if !env_cue.exists() {
            return Ok(vec![]);
        }

        // Read the CUE file and look for environments field
        let content = std::fs::read_to_string(&env_cue)
            .map_err(|e| Error::file_system(env_cue.clone(), "read", e))?;

        // Simple parsing to find environments - look for "environments: {"
        let mut environments = Vec::new();
        let mut in_environments = false;
        let mut brace_depth = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("environments:") {
                in_environments = true;
                if trimmed.contains('{') {
                    brace_depth = 1;
                }
                continue;
            }

            if in_environments {
                // Check for environment names BEFORE counting braces
                if brace_depth == 1 && trimmed.contains(':') && trimmed.contains('{') {
                    if let Some(env_name) = trimmed.split(':').next() {
                        let env_name = env_name.trim().trim_matches('"');
                        if !env_name.is_empty() {
                            environments.push(env_name.to_string());
                        }
                    }
                }

                // Count braces to track nesting
                for ch in trimmed.chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                in_environments = false;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        environments.sort();
        Ok(environments)
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
            // Canonicalize path for security
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    // If canonicalization fails, consider it not allowed
                    return Ok(DirectoryResponse {
                        allowed: false,
                        has_env_cue: false,
                    });
                }
            };

            let dir_manager = DirectoryManager::new();
            dir_manager
                .is_directory_allowed(&canonical_path)
                .unwrap_or_default()
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
        let env_cue = path.join("env.cue");

        // If no env.cue file exists, return empty list
        if !env_cue.exists() {
            return Ok(CapabilitiesResponse {
                capabilities: vec![],
            });
        }

        // Read the CUE file and look for capabilities
        let content = std::fs::read_to_string(&env_cue)
            .map_err(|e| Error::file_system(env_cue.clone(), "read", e))?;

        // Simple parsing to find capabilities
        let mut capabilities = std::collections::HashSet::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Look for capabilities arrays
            if trimmed.contains("capabilities:") && trimmed.contains('[') {
                // Extract capabilities from array syntax like: capabilities: ["build", "test"]
                if let Some(start) = trimmed.find('[') {
                    if let Some(end) = trimmed.find(']') {
                        let caps_str = &trimmed[start + 1..end];
                        for cap in caps_str.split(',') {
                            let cap = cap.trim().trim_matches('"');
                            if !cap.is_empty() {
                                capabilities.insert(cap.to_string());
                            }
                        }
                    }
                }
            }

            // Also look for single capability fields
            if trimmed.contains("capability:") && trimmed.contains('"') {
                // Extract from syntax like: capability: "app"
                if let Some(start) = trimmed.find('"') {
                    let remainder = &trimmed[start + 1..];
                    if let Some(end) = remainder.find('"') {
                        let cap = &remainder[..end];
                        if !cap.is_empty() {
                            capabilities.insert(cap.to_string());
                        }
                    }
                }
            }
        }

        let mut capabilities: Vec<String> = capabilities.into_iter().collect();
        capabilities.sort();

        Ok(CapabilitiesResponse { capabilities })
    }
}
