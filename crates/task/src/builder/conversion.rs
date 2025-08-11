//! Task configuration to definition conversion
//!
//! This module handles the conversion from TaskConfig (configuration format)
//! to TaskDefinition (runtime format) with proper validation and defaults.

use cuenv_config::TaskConfig;
use cuenv_core::{
    Error, ResolvedDependency, Result, TaskCache, TaskDefinition, TaskExecutionMode, TaskSecurity,
    DEFAULT_TASK_TIMEOUT_SECS,
};
use std::path::PathBuf;
use std::time::Duration;

/// Convert TaskConfig to TaskDefinition with validation
pub fn config_to_definition(config: TaskConfig) -> Result<TaskDefinition> {
    // Determine execution mode
    let execution_mode = create_execution_mode(&config)?;

    // Convert dependencies
    let dependencies = convert_dependencies(&config);

    // Convert security config if present
    let security = convert_security_config(&config);

    // Convert cache config
    let cache = convert_cache_config(&config);

    // Build the final task definition
    let definition = TaskDefinition {
        name: String::new(), // Will be set by caller
        description: config.description,
        execution_mode,
        dependencies,
        working_directory: PathBuf::from(config.working_dir.unwrap_or_else(|| ".".to_string())),
        shell: config.shell.unwrap_or_else(|| "sh".to_string()),
        inputs: config.inputs.unwrap_or_default(),
        outputs: config.outputs.unwrap_or_default(),
        security,
        cache,
        timeout: config
            .timeout
            .map(|t| Duration::from_secs(t as u64))
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS)),
    };

    Ok(definition)
}

/// Create the execution mode from the task configuration
fn create_execution_mode(config: &TaskConfig) -> Result<TaskExecutionMode> {
    match (&config.command, &config.script) {
        (Some(command), None) => Ok(TaskExecutionMode::Command {
            command: command.clone(),
        }),
        (None, Some(script)) => Ok(TaskExecutionMode::Script {
            content: script.clone(),
        }),
        (Some(_), Some(_)) => Err(Error::configuration(
            "Task cannot have both command and script".to_string(),
        )),
        (None, None) => Err(Error::configuration(
            "Task must have either command or script".to_string(),
        )),
    }
}

/// Convert task dependencies to resolved dependencies
fn convert_dependencies(config: &TaskConfig) -> Vec<ResolvedDependency> {
    config
        .dependencies
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|dep| ResolvedDependency::new(dep.clone()))
        .collect()
}

/// Convert security configuration to TaskSecurity
fn convert_security_config(config: &TaskConfig) -> Option<TaskSecurity> {
    config.security.as_ref().map(|sec| TaskSecurity {
        restrict_disk: sec.restrict_disk.unwrap_or(false),
        restrict_network: sec.restrict_network.unwrap_or(false),
        read_only_paths: sec
            .read_only_paths
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .map(PathBuf::from)
            .collect(),
        write_only_paths: Vec::new(), // TODO: Add when TaskConfig supports it
        allowed_hosts: sec.allowed_hosts.as_ref().unwrap_or(&Vec::new()).clone(),
    })
}

/// Convert cache configuration to TaskCache
fn convert_cache_config(config: &TaskConfig) -> TaskCache {
    match &config.cache {
        Some(_cache_config) => TaskCache {
            enabled: true, // If cache config is present, enable it
            key: config.cache_key.clone(),
            env_filter: None, // TODO: Convert from cache_config if needed
        },
        None => TaskCache::default(),
    }
}

/// Validate that the conversion produces a valid task definition
pub fn validate_conversion(definition: &TaskDefinition) -> Result<()> {
    // Ensure execution mode is properly set
    match &definition.execution_mode {
        TaskExecutionMode::Command { command } => {
            if command.trim().is_empty() {
                return Err(Error::configuration(
                    "Command cannot be empty after conversion".to_string(),
                ));
            }
        }
        TaskExecutionMode::Script { content } => {
            if content.trim().is_empty() {
                return Err(Error::configuration(
                    "Script content cannot be empty after conversion".to_string(),
                ));
            }
        }
    }

    // Validate timeout is reasonable
    if definition.timeout.as_secs() == 0 {
        return Err(Error::configuration(
            "Timeout cannot be zero after conversion".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::{SecurityConfig, TaskCacheConfig};

    fn create_basic_task_config() -> TaskConfig {
        TaskConfig {
            description: Some("Test task".to_string()),
            command: Some("echo hello".to_string()),
            script: None,
            dependencies: None,
            working_dir: None,
            shell: Some("sh".to_string()),
            inputs: None,
            outputs: None,
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: Some(30),
        }
    }

    #[test]
    fn test_basic_config_conversion() {
        let config = create_basic_task_config();
        let definition = config_to_definition(config).unwrap();

        assert_eq!(definition.description, Some("Test task".to_string()));
        assert_eq!(definition.get_execution_content(), "echo hello");
        assert_eq!(definition.shell, "sh");
        assert_eq!(definition.timeout, Duration::from_secs(30));
        assert_eq!(definition.working_directory, PathBuf::from("."));
    }

    #[test]
    fn test_script_execution_mode() {
        let mut config = create_basic_task_config();
        config.command = None;
        config.script = Some("#!/bin/bash\necho script".to_string());

        let definition = config_to_definition(config).unwrap();

        match definition.execution_mode {
            TaskExecutionMode::Script { content } => {
                assert_eq!(content, "#!/bin/bash\necho script");
            }
            _ => panic!("Expected Script execution mode"),
        }
    }

    #[test]
    fn test_both_command_and_script_error() {
        let mut config = create_basic_task_config();
        config.script = Some("echo script".to_string());

        let result = config_to_definition(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot have both"));
    }

    #[test]
    fn test_neither_command_nor_script_error() {
        let mut config = create_basic_task_config();
        config.command = None;

        let result = config_to_definition(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have either"));
    }

    #[test]
    fn test_dependencies_conversion() {
        let mut config = create_basic_task_config();
        config.dependencies = Some(vec!["dep1".to_string(), "dep2".to_string()]);

        let definition = config_to_definition(config).unwrap();

        assert_eq!(definition.dependencies.len(), 2);
        assert_eq!(definition.dependencies[0].name, "dep1");
        assert_eq!(definition.dependencies[1].name, "dep2");
    }

    #[test]
    fn test_security_config_conversion() {
        let mut config = create_basic_task_config();
        config.security = Some(SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: Some(false),
            read_only_paths: Some(vec!["./readonly".to_string()]),
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: Some(vec!["example.com".to_string()]),
            infer_from_inputs_outputs: None,
        });

        let definition = config_to_definition(config).unwrap();

        let security = definition.security.as_ref().unwrap();
        assert!(security.restrict_disk);
        assert!(!security.restrict_network);
        assert_eq!(security.read_only_paths, vec![PathBuf::from("./readonly")]);
        assert_eq!(security.allowed_hosts, vec!["example.com"]);
    }

    #[test]
    fn test_cache_config_conversion() {
        let mut config = create_basic_task_config();
        config.cache = Some(TaskCacheConfig::Simple(true));
        config.cache_key = Some("custom-key".to_string());

        let definition = config_to_definition(config).unwrap();

        assert!(definition.cache.enabled);
        assert_eq!(definition.cache.key, Some("custom-key".to_string()));
    }

    #[test]
    fn test_default_values() {
        let config = TaskConfig {
            description: None,
            command: Some("echo test".to_string()),
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: None,
            outputs: None,
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: None,
        };

        let definition = config_to_definition(config).unwrap();

        assert!(definition.description.is_none());
        assert_eq!(definition.shell, "sh");
        assert_eq!(definition.working_directory, PathBuf::from("."));
        assert_eq!(
            definition.timeout,
            Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS)
        );
        assert!(definition.inputs.is_empty());
        assert!(definition.outputs.is_empty());
        assert!(!definition.cache.enabled);
    }

    #[test]
    fn test_validate_conversion_success() {
        let config = create_basic_task_config();
        let definition = config_to_definition(config).unwrap();

        let result = validate_conversion(&definition);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_conversion_empty_command() {
        let mut config = create_basic_task_config();
        config.command = Some("   ".to_string()); // Only whitespace

        let definition = config_to_definition(config).unwrap();
        let result = validate_conversion(&definition);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_custom_working_directory() {
        let mut config = create_basic_task_config();
        config.working_dir = Some("./custom/dir".to_string());

        let definition = config_to_definition(config).unwrap();
        assert_eq!(definition.working_directory, PathBuf::from("./custom/dir"));
    }

    #[test]
    fn test_custom_timeout() {
        let mut config = create_basic_task_config();
        config.timeout = Some(120);

        let definition = config_to_definition(config).unwrap();
        assert_eq!(definition.timeout, Duration::from_secs(120));
    }
}
