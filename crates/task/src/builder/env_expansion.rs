//! Environment variable expansion for task building
//!
//! This module handles expansion of environment variables in task commands and scripts
//! using the ${VAR} syntax pattern.

use cuenv_core::{Result, TaskExecutionMode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::BuildContext;

/// Expand environment variables in task execution content and working directories
pub fn expand_environment_variables(
    context: &mut BuildContext,
    global_env: &HashMap<String, String>,
) -> Result<()> {
    for definition in context.task_definitions.values_mut() {
        // Expand environment variables in execution content
        match &mut definition.execution_mode {
            TaskExecutionMode::Command { command } => {
                *command = expand_env_vars(command, global_env)?;
            }
            TaskExecutionMode::Script { content } => {
                *content = expand_env_vars(content, global_env)?;
            }
        }
    }

    Ok(())
}

/// Resolve working directories to absolute paths with environment variable expansion
pub fn resolve_working_directories(
    context: &mut BuildContext,
    workspace_root: &Path,
    global_env: &HashMap<String, String>,
) -> Result<()> {
    for definition in context.task_definitions.values_mut() {
        // First expand any environment variables in the working directory path
        let working_dir_str = definition.working_directory.to_string_lossy();
        let expanded_path = expand_env_vars(&working_dir_str, global_env)?;
        definition.working_directory = PathBuf::from(expanded_path);

        // If working_directory is relative, make it relative to workspace_root
        if !definition.working_directory.is_absolute() {
            definition.working_directory = workspace_root
                .join(&definition.working_directory)
                .canonicalize()
                .map_err(|e| {
                    cuenv_core::Error::configuration(format!(
                        "Failed to resolve working directory '{}' for task '{}': {}",
                        definition.working_directory.display(),
                        definition.name,
                        e
                    ))
                })?;
        }
    }

    Ok(())
}

/// Expand environment variables in a string using ${VAR} syntax
pub fn expand_env_vars(input: &str, env_vars: &HashMap<String, String>) -> Result<String> {
    let mut result = input.to_string();
    let mut start = 0;

    while let Some(pos) = result[start..].find("${") {
        let abs_pos = start + pos;

        if let Some(end_pos) = result[abs_pos + 2..].find('}') {
            let var_name = &result[abs_pos + 2..abs_pos + 2 + end_pos];

            // Look up the variable in the environment
            let var_value = env_vars.get(var_name).map(|s| s.as_str()).unwrap_or("");

            // Replace ${VAR} with the value
            result.replace_range(abs_pos..abs_pos + 3 + end_pos, var_value);
            start = abs_pos + var_value.len();
        } else {
            // No closing brace found, skip this occurrence
            start = abs_pos + 2;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_core::TaskDefinition;
    use std::time::Duration;

    fn create_test_definition(name: &str, command: &str, working_dir: &str) -> TaskDefinition {
        TaskDefinition {
            name: name.to_string(),
            description: Some("Test task".to_string()),
            execution_mode: TaskExecutionMode::Command {
                command: command.to_string(),
            },
            dependencies: Vec::new(),
            working_directory: PathBuf::from(working_dir),
            shell: "sh".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            security: None,
            cache: cuenv_core::TaskCache::default(),
            timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn test_expand_env_vars_basic() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "expanded_value".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());

        let result = expand_env_vars("echo ${TEST_VAR} in ${HOME}", &env).unwrap();
        assert_eq!(result, "echo expanded_value in /home/user");
    }

    #[test]
    fn test_expand_env_vars_missing() {
        let env = HashMap::new();
        let result = expand_env_vars("echo ${MISSING_VAR}", &env).unwrap();
        assert_eq!(result, "echo ");
    }

    #[test]
    fn test_expand_env_vars_no_closing_brace() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "value".to_string());

        let result = expand_env_vars("echo ${TEST_VAR} and ${UNCLOSED", &env).unwrap();
        assert_eq!(result, "echo value and ${UNCLOSED");
    }

    #[test]
    fn test_expand_env_vars_nested() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());
        env.insert("VAR2".to_string(), "value2".to_string());

        let result = expand_env_vars("${VAR1}-${VAR2}-${VAR1}", &env).unwrap();
        assert_eq!(result, "value1-value2-value1");
    }

    #[test]
    fn test_expand_environment_variables_in_context() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_nodes: indexmap::IndexMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context.task_definitions.insert(
            "test".to_string(),
            create_test_definition("test", "echo ${TEST_VAR}", "."),
        );

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "expanded".to_string());

        let result = expand_environment_variables(&mut context, &env);
        assert!(result.is_ok());

        let definition = &context.task_definitions["test"];
        assert_eq!(definition.get_execution_content(), "echo expanded");
    }

    #[test]
    fn test_expand_script_content() {
        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_nodes: indexmap::IndexMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        let mut definition = create_test_definition("test", "", ".");
        definition.execution_mode = TaskExecutionMode::Script {
            content: "#!/bin/bash\necho ${TEST_VAR}".to_string(),
        };
        context
            .task_definitions
            .insert("test".to_string(), definition);

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "script_value".to_string());

        let result = expand_environment_variables(&mut context, &env);
        assert!(result.is_ok());

        let definition = &context.task_definitions["test"];
        assert_eq!(
            definition.get_execution_content(),
            "#!/bin/bash\necho script_value"
        );
    }

    #[test]
    fn test_resolve_working_directories_with_env() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        let mut context = BuildContext {
            task_configs: HashMap::new(),
            task_nodes: indexmap::IndexMap::new(),
            task_definitions: HashMap::new(),
            dependency_graph: HashMap::new(),
        };

        context.task_definitions.insert(
            "test".to_string(),
            create_test_definition("test", "echo hello", "${HOME}/subdir"),
        );

        let mut env = HashMap::new();
        env.insert(
            "HOME".to_string(),
            temp_dir
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        );

        // Create the subdir for canonicalization to work
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        let result = resolve_working_directories(&mut context, &workspace_root, &env);
        assert!(result.is_ok());

        let definition = &context.task_definitions["test"];
        assert_eq!(
            definition.working_directory,
            sub_dir.canonicalize().unwrap()
        );
    }
}
