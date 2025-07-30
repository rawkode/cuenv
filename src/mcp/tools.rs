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
pub struct CuenvToolBox {
    pub allow_exec: bool,
}

/// Validate directory and check if it's allowed
fn validate_directory(directory: &str) -> Result<PathBuf> {
    let path = PathBuf::from(directory);

    // Canonicalize path to prevent path traversal attacks
    let canonical_path = path.canonicalize().map_err(|e| {
        Error::configuration(format!(
            "Cannot canonicalize directory path '{}': {}. Directory may not exist or be accessible.",
            directory, e
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
        // We need to parse the raw CUE result to access environment data
        use std::ffi::CString;
        use std::os::raw::c_char;

        // Use FFI directly to get the raw JSON that includes environment data
        extern "C" {
            fn cue_eval_package(
                path: *const c_char,
                package_name: *const c_char,
                environment: *const c_char,
                capabilities: *const c_char,
            ) -> *mut c_char;
            fn cue_free_string(ptr: *mut c_char);
        }

        let path_cstr = CString::new(path.to_string_lossy().as_ref())
            .map_err(|e| Error::configuration(format!("Invalid path: {}", e)))?;
        let package_cstr = CString::new("env")
            .map_err(|e| Error::configuration(format!("Invalid package name: {}", e)))?;
        let env_cstr = CString::new("")
            .map_err(|e| Error::configuration(format!("Invalid environment: {}", e)))?;
        let cap_cstr = CString::new("")
            .map_err(|e| Error::configuration(format!("Invalid capabilities: {}", e)))?;

        // Call FFI to get raw result
        let result_ptr = unsafe {
            cue_eval_package(
                path_cstr.as_ptr(),
                package_cstr.as_ptr(),
                env_cstr.as_ptr(),
                cap_cstr.as_ptr(),
            )
        };

        if result_ptr.is_null() {
            return Ok(vec![]);
        }

        // Convert to string and parse JSON
        let json_str = unsafe {
            let cstr = std::ffi::CStr::from_ptr(result_ptr);
            cstr.to_str().map_err(|e| {
                Error::configuration(format!("Failed to convert result to UTF-8: {}", e))
            })?
        };

        let json_value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| Error::configuration(format!("Failed to parse JSON: {}", e)))?;

        // Free the C string
        unsafe {
            cue_free_string(result_ptr);
        }

        // Extract environment names from the "environments" field if it exists
        let environments =
            if let Some(envs) = json_value.get("environments").and_then(|v| v.as_object()) {
                envs.keys().cloned().collect()
            } else {
                // Fallback: check if env.cue exists and return empty list if no environments found
                let env_cue = path.join("env.cue");
                if env_cue.exists() {
                    vec![]
                } else {
                    vec![]
                }
            };

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
            match dir_manager.is_directory_allowed(&canonical_path) {
                Ok(allowed) => allowed,
                Err(_) => false, // If permission check fails, consider it not allowed
            }
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

        // Use FFI directly to get the raw JSON that includes metadata with capabilities
        use std::ffi::CString;
        use std::os::raw::c_char;

        extern "C" {
            fn cue_eval_package(
                path: *const c_char,
                package_name: *const c_char,
                environment: *const c_char,
                capabilities: *const c_char,
            ) -> *mut c_char;
            fn cue_free_string(ptr: *mut c_char);
        }

        let path_cstr = CString::new(path.to_string_lossy().as_ref())
            .map_err(|e| Error::configuration(format!("Invalid path: {}", e)))?;
        let package_cstr = CString::new("env")
            .map_err(|e| Error::configuration(format!("Invalid package name: {}", e)))?;
        let env_cstr = CString::new("")
            .map_err(|e| Error::configuration(format!("Invalid environment: {}", e)))?;
        let cap_cstr = CString::new("")
            .map_err(|e| Error::configuration(format!("Invalid capabilities: {}", e)))?;

        // Call FFI to get raw result
        let result_ptr = unsafe {
            cue_eval_package(
                path_cstr.as_ptr(),
                package_cstr.as_ptr(),
                env_cstr.as_ptr(),
                cap_cstr.as_ptr(),
            )
        };

        if result_ptr.is_null() {
            return Ok(CapabilitiesResponse {
                capabilities: vec![],
            });
        }

        // Convert to string and parse JSON
        let json_str = unsafe {
            let cstr = std::ffi::CStr::from_ptr(result_ptr);
            cstr.to_str().map_err(|e| {
                Error::configuration(format!("Failed to convert result to UTF-8: {}", e))
            })?
        };

        let json_value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| Error::configuration(format!("Failed to parse JSON: {}", e)))?;

        // Free the C string
        unsafe {
            cue_free_string(result_ptr);
        }

        // Extract unique capabilities from metadata
        let mut capabilities = std::collections::HashSet::new();

        // Check variable metadata for capabilities
        if let Some(metadata) = json_value.get("metadata").and_then(|v| v.as_object()) {
            for (_, var_meta) in metadata {
                if let Some(capability) = var_meta.get("capability").and_then(|v| v.as_str()) {
                    capabilities.insert(capability.to_string());
                }
            }
        }

        // Check command capabilities
        if let Some(commands) = json_value.get("commands").and_then(|v| v.as_object()) {
            for (_, cmd_config) in commands {
                if let Some(caps) = cmd_config.get("capabilities").and_then(|v| v.as_array()) {
                    for cap in caps {
                        if let Some(cap_str) = cap.as_str() {
                            capabilities.insert(cap_str.to_string());
                        }
                    }
                }
            }
        }

        // Check task capabilities
        if let Some(tasks) = json_value.get("tasks").and_then(|v| v.as_object()) {
            for (_, task_config) in tasks {
                if let Some(caps) = task_config.get("capabilities").and_then(|v| v.as_array()) {
                    for cap in caps {
                        if let Some(cap_str) = cap.as_str() {
                            capabilities.insert(cap_str.to_string());
                        }
                    }
                }
            }
        }

        Ok(CapabilitiesResponse {
            capabilities: capabilities.into_iter().collect(),
        })
    }
}
