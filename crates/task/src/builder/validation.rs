//! Task validation logic for the TaskBuilder
//!
//! This module provides validation functionality for task configurations,
//! ensuring they meet the required constraints and standards.

use cuenv_config::TaskConfig;
use cuenv_core::{Error, Result};
use std::collections::HashMap;

/// Validates basic task configurations
pub fn validate_task_configs(task_configs: &HashMap<String, TaskConfig>) -> Result<()> {
    for (name, config) in task_configs {
        // Validate task name
        if name.is_empty() {
            return Err(Error::configuration(
                "Task name cannot be empty".to_string(),
            ));
        }

        // Validate command/script exclusivity
        validate_command_script_exclusivity(name, config)?;

        // Validate shell
        if let Some(shell) = &config.shell {
            validate_shell(shell)?;
        }

        // Validate timeout
        if let Some(timeout) = config.timeout {
            if timeout == 0 {
                return Err(Error::configuration(format!(
                    "Task '{name}' timeout must be greater than 0"
                )));
            }
        }
    }

    Ok(())
}

/// Validate that command and script are mutually exclusive
fn validate_command_script_exclusivity(name: &str, config: &TaskConfig) -> Result<()> {
    match (&config.command, &config.script) {
        (Some(_), Some(_)) => Err(Error::configuration(format!(
            "Task '{name}' cannot have both 'command' and 'script' defined"
        ))),
        (None, None) => Err(Error::configuration(format!(
            "Task '{name}' must have either 'command' or 'script' defined"
        ))),
        _ => Ok(()), // Valid
    }
}

/// Validate shell command
pub fn validate_shell(shell: &str) -> Result<()> {
    const ALLOWED_SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "pwsh", "powershell"];

    if !ALLOWED_SHELLS.contains(&shell) {
        return Err(Error::configuration(format!(
            "Shell '{}' is not allowed. Allowed shells: {}",
            shell,
            ALLOWED_SHELLS.join(", ")
        )));
    }

    // Check if shell is available on the system (best effort)
    if std::process::Command::new("which")
        .arg(shell)
        .output()
        .is_err()
    {
        tracing::warn!("Shell '{}' may not be available on this system", shell);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::TaskConfig;

    fn create_test_config(command: Option<&str>, script: Option<&str>) -> TaskConfig {
        TaskConfig {
            description: Some("Test task".to_string()),
            command: command.map(|s| s.to_string()),
            script: script.map(|s| s.to_string()),
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
    fn test_valid_task_configs() {
        let mut configs = HashMap::new();
        configs.insert(
            "test1".to_string(),
            create_test_config(Some("echo hello"), None),
        );
        configs.insert(
            "test2".to_string(),
            create_test_config(None, Some("echo script")),
        );

        let result = validate_task_configs(&configs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_task_name() {
        let mut configs = HashMap::new();
        configs.insert("".to_string(), create_test_config(Some("echo hello"), None));

        let result = validate_task_configs(&configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_both_command_and_script() {
        let mut configs = HashMap::new();
        configs.insert(
            "test".to_string(),
            create_test_config(Some("echo hello"), Some("echo script")),
        );

        let result = validate_task_configs(&configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot have both"));
    }

    #[test]
    fn test_neither_command_nor_script() {
        let mut configs = HashMap::new();
        configs.insert("test".to_string(), create_test_config(None, None));

        let result = validate_task_configs(&configs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have either"));
    }

    #[test]
    fn test_invalid_shell() {
        let result = validate_shell("evil_shell");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_valid_shell() {
        let result = validate_shell("bash");
        assert!(result.is_ok());
    }

    #[test]
    fn test_zero_timeout() {
        let mut configs = HashMap::new();
        let mut config = create_test_config(Some("echo hello"), None);
        config.timeout = Some(0);
        configs.insert("test".to_string(), config);

        let result = validate_task_configs(&configs);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be greater than 0"));
    }
}
