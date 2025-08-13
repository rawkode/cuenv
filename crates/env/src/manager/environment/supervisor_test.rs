#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::Hook;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    fn create_test_hook(command: &str, args: Vec<String>, preload: bool, source: bool) -> Hook {
        Hook {
            command: command.to_string(),
            args: Some(args),
            dir: None,
            preload: Some(preload),
            source: Some(source),
            inputs: None,
            outputs: None,
            cache: None,
            when: None,
        }
    }

    fn create_test_hook_with_inputs(command: &str, args: Vec<String>, inputs: Vec<String>) -> Hook {
        Hook {
            command: command.to_string(),
            args: Some(args),
            dir: None,
            preload: Some(true),
            source: Some(false),
            inputs: Some(inputs),
            outputs: None,
            cache: None,
            when: None,
        }
    }

    #[test]
    fn test_parse_env_line() {
        // Test basic KEY=VALUE
        assert_eq!(
            parse_env_line("FOO=bar"),
            Some(("FOO".to_string(), "bar".to_string()))
        );

        // Test export KEY=VALUE
        assert_eq!(
            parse_env_line("export FOO=bar"),
            Some(("FOO".to_string(), "bar".to_string()))
        );

        // Test quoted values
        assert_eq!(
            parse_env_line("FOO=\"bar baz\""),
            Some(("FOO".to_string(), "bar baz".to_string()))
        );

        assert_eq!(
            parse_env_line("FOO='bar baz'"),
            Some(("FOO".to_string(), "bar baz".to_string()))
        );

        // Test comments and empty lines
        assert_eq!(parse_env_line("# comment"), None);
        assert_eq!(parse_env_line(""), None);
        assert_eq!(parse_env_line("   "), None);

        // Test whitespace handling
        assert_eq!(
            parse_env_line("  export FOO=bar  "),
            Some(("FOO".to_string(), "bar".to_string()))
        );
    }

    #[tokio::test]
    async fn test_calculate_input_hash_consistency() {
        let hooks = vec![
            create_test_hook("echo", vec!["hello".to_string()], true, false),
            create_test_hook("echo", vec!["world".to_string()], true, false),
        ];

        let hash1 = calculate_input_hash(&hooks).unwrap();
        let hash2 = calculate_input_hash(&hooks).unwrap();

        assert_eq!(hash1, hash2, "Hash should be consistent for same inputs");
    }

    #[tokio::test]
    async fn test_calculate_input_hash_changes_with_inputs() {
        let hooks1 = vec![create_test_hook(
            "echo",
            vec!["hello".to_string()],
            true,
            false,
        )];
        let hooks2 = vec![create_test_hook(
            "echo",
            vec!["world".to_string()],
            true,
            false,
        )];

        let hash1 = calculate_input_hash(&hooks1).unwrap();
        let hash2 = calculate_input_hash(&hooks2).unwrap();

        assert_ne!(hash1, hash2, "Hash should change with different inputs");
    }

    #[tokio::test]
    async fn test_calculate_input_hash_with_file_inputs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "initial content").unwrap();

        let hooks = vec![create_test_hook_with_inputs(
            "echo",
            vec!["test".to_string()],
            vec![file_path.to_string_lossy().to_string()],
        )];

        let hash1 = calculate_input_hash(&hooks).unwrap();

        // Modify the file
        sleep(Duration::from_millis(10)).await;
        fs::write(&file_path, "modified content").unwrap();

        let hash2 = calculate_input_hash(&hooks).unwrap();

        assert_ne!(
            hash1, hash2,
            "Hash should change when input file is modified"
        );
    }

    #[tokio::test]
    async fn test_captured_environment_serialization() {
        let mut env_vars = HashMap::new();
        env_vars.insert("FOO".to_string(), "bar".to_string());
        env_vars.insert("BAZ".to_string(), "qux".to_string());

        let captured = CapturedEnvironment {
            env_vars: env_vars.clone(),
            input_hash: "test_hash".to_string(),
            timestamp: 12345,
        };

        // Serialize
        let json = serde_json::to_string(&captured).unwrap();

        // Deserialize
        let deserialized: CapturedEnvironment = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.input_hash, "test_hash");
        assert_eq!(deserialized.timestamp, 12345);
        assert_eq!(deserialized.env_vars, env_vars);
    }

    #[tokio::test]
    async fn test_execute_hook_with_timeout_success() {
        let hook = create_test_hook("echo", vec!["hello".to_string()], true, false);
        let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

        assert!(result.is_ok(), "Echo command should succeed");
        assert_eq!(result.unwrap(), None, "Non-source hook should return None");
    }

    #[tokio::test]
    async fn test_execute_hook_with_timeout_failure() {
        let hook = create_test_hook("false", vec![], true, false);
        let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

        assert!(result.is_err(), "False command should fail");
    }

    #[tokio::test]
    async fn test_execute_hook_with_timeout_actual_timeout() {
        let hook = create_test_hook("sleep", vec!["10".to_string()], true, false);
        let result = execute_hook_with_timeout(&hook, Duration::from_millis(100)).await;

        assert!(result.is_err(), "Sleep command should timeout");
        assert!(
            result.unwrap_err().to_string().contains("timed out"),
            "Error should mention timeout"
        );
    }

    #[tokio::test]
    async fn test_execute_source_hook() {
        // Create a script that outputs environment variables
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("source.sh");
        fs::write(
            &script_path,
            "#!/bin/sh
echo \"export FOO=bar\"
echo \"export BAZ=qux\"
echo \"# This is a comment\"
echo \"\"
echo \"HELLO=world\"
",
        )
        .unwrap();

        // Make script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let hook = Hook {
            command: script_path.to_string_lossy().to_string(),
            args: None,
            dir: None,
            preload: Some(true),
            source: Some(true),
            inputs: None,
            outputs: None,
            cache: None,
            when: None,
        };

        let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

        assert!(result.is_ok(), "Source hook should succeed");
        let env_vars = result.unwrap().unwrap();

        assert_eq!(env_vars.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(env_vars.get("BAZ"), Some(&"qux".to_string()));
        assert_eq!(env_vars.get("HELLO"), Some(&"world".to_string()));
    }

    #[tokio::test]
    async fn test_supervisor_no_preload_hooks() {
        let hooks = vec![
            create_test_hook("echo", vec!["test".to_string()], false, false), // Not a preload hook
        ];

        let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
        let result = supervisor.run().await;

        assert!(
            result.is_ok(),
            "Supervisor should succeed with no preload hooks"
        );
    }

    #[tokio::test]
    async fn test_supervisor_with_cache() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let hooks = vec![create_test_hook_with_inputs(
            "echo",
            vec!["cached".to_string()],
            vec![test_file.to_string_lossy().to_string()],
        )];

        // First run - should execute hooks
        let supervisor1 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
        let result1 = supervisor1.run().await;
        assert!(result1.is_ok());

        // Second run with same inputs - should use cache
        let supervisor2 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
        let result2 = supervisor2.run().await;
        assert!(result2.is_ok());

        // Modify input file
        fs::write(&test_file, "modified content").unwrap();

        // Third run with modified inputs - should re-execute
        let supervisor3 = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
        let result3 = supervisor3.run().await;
        assert!(result3.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_status_tracking() {
        let hooks = vec![
            create_test_hook("echo", vec!["1".to_string()], true, false),
            create_test_hook("echo", vec!["2".to_string()], true, false),
            create_test_hook("echo", vec!["3".to_string()], true, false),
        ];

        let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();

        // Check initial status
        let initial_status = supervisor.status_manager.get_current_status();
        assert_eq!(initial_status.total, 0);

        let result = supervisor.run().await;
        assert!(result.is_ok());

        // After completion, status should be cleared
        let final_status = supervisor.status_manager.get_current_status();
        assert_eq!(
            final_status.total, 0,
            "Status should be cleared after completion"
        );
    }

    #[tokio::test]
    async fn test_supervisor_handles_hook_failure() {
        let hooks = vec![
            create_test_hook("echo", vec!["success".to_string()], true, false),
            create_test_hook("false", vec![], true, false), // This will fail
            create_test_hook("echo", vec!["after_failure".to_string()], true, false),
        ];

        let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
        let result = supervisor.run().await;

        // Supervisor should continue despite individual hook failures
        assert!(
            result.is_ok(),
            "Supervisor should complete even with failed hooks"
        );
    }

    #[tokio::test]
    async fn test_cache_directory_creation() {
        let cache_dir = get_cache_dir().unwrap();
        assert!(cache_dir.to_string_lossy().contains("cuenv"));
        assert!(cache_dir.to_string_lossy().contains("preload-cache"));
    }

    #[tokio::test]
    async fn test_captured_environment_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let captured = CapturedEnvironment {
            env_vars: env_vars.clone(),
            input_hash: "test_hash_123".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Save to file
        let cache_file = cache_dir.join("test_hash_123.json");
        let content = serde_json::to_string_pretty(&captured).unwrap();
        fs::write(&cache_file, content).unwrap();

        // Load from file
        let loaded_content = fs::read_to_string(&cache_file).unwrap();
        let loaded: CapturedEnvironment = serde_json::from_str(&loaded_content).unwrap();

        assert_eq!(loaded.input_hash, captured.input_hash);
        assert_eq!(loaded.env_vars, captured.env_vars);
    }
}
