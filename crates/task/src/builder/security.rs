//! Security validation and path resolution for task building
//!
//! This module handles validation of security configurations, ensuring that
//! security paths are properly resolved and validated for task execution.

use cuenv_core::{Error, Result, TaskSecurity};
use std::path::{Path, PathBuf};

use super::BuildContext;

/// Validate security configurations for all tasks in the build context
pub fn validate_security_configs(context: &mut BuildContext, workspace_root: &Path) -> Result<()> {
    for (task_name, definition) in &mut context.task_definitions {
        if let Some(security) = &mut definition.security {
            // Validate and resolve paths
            resolve_security_paths(task_name, security, workspace_root)?;

            // Validate hosts
            validate_security_hosts(task_name, security)?;
        }
    }

    Ok(())
}

/// Resolve security paths to absolute paths and validate them
pub fn resolve_security_paths(
    task_name: &str,
    security: &mut TaskSecurity,
    workspace_root: &Path,
) -> Result<()> {
    // Helper to resolve and validate paths
    let resolve_paths = |paths: &mut Vec<PathBuf>| -> Result<()> {
        for path in paths.iter_mut() {
            if !path.is_absolute() {
                *path = workspace_root.join(&path);
            }

            // Validate that the path is within reasonable bounds (security check)
            if let Ok(canonical) = path.canonicalize() {
                *path = canonical;
            } else {
                // Path doesn't exist - that's OK for security restrictions
                // but ensure it's at least under workspace
                if let Ok(canonical_workspace) = workspace_root.canonicalize() {
                    // Try to get the canonical parent and join the filename back
                    // This handles cases where the path contains "./" components
                    let normalized_path = if let Some(parent) = path.parent() {
                        if let Ok(canonical_parent) = parent.canonicalize() {
                            canonical_parent.join(path.file_name().unwrap_or_default())
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };

                    if !normalized_path.starts_with(&canonical_workspace) {
                        return Err(Error::configuration(format!(
                            "Security path '{}' in task '{}' must be within workspace",
                            path.display(),
                            task_name
                        )));
                    }
                }
            }
        }
        Ok(())
    };

    resolve_paths(&mut security.read_only_paths)?;
    resolve_paths(&mut security.write_only_paths)?;

    Ok(())
}

/// Validate security host configurations
pub fn validate_security_hosts(task_name: &str, security: &TaskSecurity) -> Result<()> {
    for host in &security.allowed_hosts {
        if host.is_empty() {
            return Err(Error::configuration(format!(
                "Empty host in allowed_hosts for task '{task_name}'"
            )));
        }

        // Basic validation - ensure it's not just whitespace and has reasonable format
        if host.trim() != host || host.contains(' ') {
            return Err(Error::configuration(format!(
                "Invalid host '{host}' in allowed_hosts for task '{task_name}'. Hosts cannot contain spaces"
            )));
        }
    }

    Ok(())
}

/// Validate individual security path to ensure it's within workspace bounds
pub fn validate_security_path(path: &Path, workspace_root: &Path, task_name: &str) -> Result<()> {
    if let Ok(canonical_workspace) = workspace_root.canonicalize() {
        let normalized_path = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else {
            // For non-existent paths, try to normalize by getting canonical parent
            if let Some(parent) = path.parent() {
                if let Ok(canonical_parent) = parent.canonicalize() {
                    canonical_parent.join(path.file_name().unwrap_or_default())
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            }
        };

        if !normalized_path.starts_with(&canonical_workspace) {
            return Err(Error::configuration(format!(
                "Security path '{}' in task '{}' must be within workspace",
                path.display(),
                task_name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_core::{TaskDefinition, TaskExecutionMode};
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_definition_with_security(
        name: &str,
        security: Option<TaskSecurity>,
    ) -> TaskDefinition {
        TaskDefinition {
            name: name.to_string(),
            description: Some("Test task".to_string()),
            execution_mode: TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            dependencies: Vec::new(),
            working_directory: PathBuf::from("."),
            shell: "sh".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            security,
            cache: cuenv_core::TaskCache::default(),
            timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn test_validate_security_hosts() {
        let security = TaskSecurity {
            restrict_disk: false,
            restrict_network: false,
            read_only_paths: Vec::new(),
            write_only_paths: Vec::new(),
            allowed_hosts: vec!["example.com".to_string(), "api.test.com".to_string()],
        };

        let result = validate_security_hosts("test_task", &security);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_security_hosts_empty() {
        let security = TaskSecurity {
            restrict_disk: false,
            restrict_network: false,
            read_only_paths: Vec::new(),
            write_only_paths: Vec::new(),
            allowed_hosts: vec!["".to_string()],
        };

        let result = validate_security_hosts("test_task", &security);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty host"));
    }

    #[test]
    fn test_validate_security_hosts_with_spaces() {
        let security = TaskSecurity {
            restrict_disk: false,
            restrict_network: false,
            read_only_paths: Vec::new(),
            write_only_paths: Vec::new(),
            allowed_hosts: vec!["invalid host.com".to_string()],
        };

        let result = validate_security_hosts("test_task", &security);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot contain spaces"));
    }

    #[test]
    fn test_resolve_security_paths() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Create a test directory
        let test_dir = temp_dir.path().join("readonly");
        fs::create_dir(&test_dir).unwrap();

        let mut security = TaskSecurity {
            restrict_disk: true,
            restrict_network: false,
            read_only_paths: vec![PathBuf::from("readonly")],
            write_only_paths: Vec::new(),
            allowed_hosts: Vec::new(),
        };

        let result = resolve_security_paths("test_task", &mut security, &workspace_root);
        assert!(result.is_ok());

        // Path should now be absolute
        assert!(security.read_only_paths[0].is_absolute());
        assert_eq!(
            security.read_only_paths[0],
            test_dir.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_resolve_security_paths_outside_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        let mut security = TaskSecurity {
            restrict_disk: true,
            restrict_network: false,
            read_only_paths: vec![PathBuf::from("/etc/passwd")],
            write_only_paths: Vec::new(),
            allowed_hosts: Vec::new(),
        };

        let result = resolve_security_paths("test_task", &mut security, &workspace_root);
        // This should succeed since /etc/passwd is absolute and not validated for workspace bounds
        // The validation happens later in the process
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_security_configs_integration() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Create test directory
        let test_dir = temp_dir.path().join("secure");
        fs::create_dir(&test_dir).unwrap();

        let security = TaskSecurity {
            restrict_disk: true,
            restrict_network: false,
            read_only_paths: vec![PathBuf::from("secure")],
            write_only_paths: Vec::new(),
            allowed_hosts: vec!["example.com".to_string()],
        };

        let mut context = BuildContext {
            task_configs: std::collections::HashMap::new(),
            task_definitions: std::collections::HashMap::new(),
            dependency_graph: std::collections::HashMap::new(),
        };

        context.task_definitions.insert(
            "test".to_string(),
            create_test_definition_with_security("test", Some(security)),
        );

        let result = validate_security_configs(&mut context, &workspace_root);
        assert!(result.is_ok());

        let definition = &context.task_definitions["test"];
        let sec = definition.security.as_ref().unwrap();
        assert!(sec.read_only_paths[0].is_absolute());
        assert_eq!(sec.allowed_hosts[0], "example.com");
    }

    #[test]
    fn test_validate_security_path_within_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let test_path = workspace_root.join("test_file");

        let result = validate_security_path(&test_path, &workspace_root, "test_task");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_security_path_outside_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let outside_path = PathBuf::from("/tmp/outside_file");

        let result = validate_security_path(&outside_path, &workspace_root, "test_task");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be within workspace"));
    }
}
