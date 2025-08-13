use super::cache::{calculate_input_hash, CapturedEnvironment};
use super::core::{Supervisor, SupervisorMode};
use super::execution::execute_hook_with_timeout;
use super::utils::get_cache_dir;
use cuenv_config::Hook;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

fn create_test_hook(command: &str, args: Vec<String>, preload: bool, source: bool) -> Hook {
    Hook {
        command: command.to_string(),
        args: Some(args),
        dir: None,
        preload: Some(preload),
        source: Some(source),
        inputs: None,
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
    }
}

#[tokio::test]
async fn test_input_hash_consistency() {
    let hooks = vec![create_test_hook(
        "echo",
        vec!["test".to_string()],
        true,
        false,
    )];

    let hash1 = calculate_input_hash(&hooks).unwrap();
    let hash2 = calculate_input_hash(&hooks).unwrap();

    assert_eq!(hash1, hash2, "Hash should be consistent for same inputs");
}

#[tokio::test]
async fn test_input_hash_different_commands() {
    let hooks1 = vec![create_test_hook(
        "echo",
        vec!["test".to_string()],
        true,
        false,
    )];
    let hooks2 = vec![create_test_hook(
        "printf",
        vec!["test".to_string()],
        true,
        false,
    )];

    let hash1 = calculate_input_hash(&hooks1).unwrap();
    let hash2 = calculate_input_hash(&hooks2).unwrap();

    assert_ne!(hash1, hash2, "Hash should differ for different commands");
}

#[tokio::test]
async fn test_input_hash_with_file_changes() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "initial content").unwrap();

    let hooks = vec![create_test_hook_with_inputs(
        "cat",
        vec![file_path.to_string_lossy().to_string()],
        vec![file_path.to_string_lossy().to_string()],
    )];

    let hash1 = calculate_input_hash(&hooks).unwrap();

    // Modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write(&file_path, "modified content").unwrap();

    let hash2 = calculate_input_hash(&hooks).unwrap();

    // Hashes should be the same since we're not monitoring file contents in inputs
    // (inputs just tracks paths, not contents)
    assert_eq!(hash1, hash2);
}

#[tokio::test]
async fn test_captured_environment_serialization() {
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

    let captured = CapturedEnvironment {
        env_vars,
        input_hash: "test_hash".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let json = serde_json::to_string(&captured).unwrap();
    assert!(json.contains("TEST_VAR"));

    let deserialized: CapturedEnvironment = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.input_hash, "test_hash");
}

#[tokio::test]
async fn test_execute_hook_non_source() {
    let hook = create_test_hook("echo", vec!["hello".to_string()], false, false);
    let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().0,
        None,
        "Non-source hook should return None"
    );
}

#[tokio::test]
async fn test_execute_hook_failure() {
    let hook = create_test_hook("false", vec![], false, false);
    let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;
    assert!(result.is_ok(), "Should handle command failure gracefully");
}

#[tokio::test]
async fn test_execute_hook_timeout() {
    let hook = create_test_hook("sleep", vec!["10".to_string()], false, false);
    let result = execute_hook_with_timeout(&hook, Duration::from_millis(100)).await;

    // Should not error, just return None for timed out hooks
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore = "Flaky in CI - needs investigation"]
async fn test_execute_source_hook() {
    let temp_dir = TempDir::new().unwrap();
    let script_path = temp_dir.path().join("test_script.sh");

    // Create a script that outputs environment variable exports
    let script_content = r#"#!/usr/bin/env bash
echo 'export TEST_VAR1="value1"'
echo 'export TEST_VAR2="value2"'
"#;

    fs::write(&script_path, script_content).unwrap();

    // Make script executable on Unix
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
        source: Some(true),
        preload: Some(false),
        inputs: None,
    };

    let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

    assert!(result.is_ok());
    let env_vars = result.unwrap().0.unwrap();
    assert_eq!(env_vars.get("TEST_VAR1"), Some(&"value1".to_string()));
    assert_eq!(env_vars.get("TEST_VAR2"), Some(&"value2".to_string()));
}

#[tokio::test]
#[ignore = "Flaky in CI - needs investigation"]  
async fn test_supervisor_background_mode() {
    let hooks = vec![create_test_hook(
        "echo",
        vec!["test".to_string()],
        false,
        false,
    )];
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();

    let result = supervisor.run().await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore = "Flaky in CI - needs investigation"]
async fn test_supervisor_with_cached_environment() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content").unwrap();

    let hooks = vec![create_test_hook_with_inputs(
        "cat",
        vec![test_file.to_string_lossy().to_string()],
        vec![test_file.to_string_lossy().to_string()],
    )];

    // Run supervisor first time
    let supervisor1 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
    supervisor1.run().await.unwrap();

    // Run again - should use cache
    let supervisor2 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
    supervisor2.run().await.unwrap();

    // Modify file
    fs::write(&test_file, "modified content").unwrap();

    // Run again - cache should still be used since we don't monitor file contents
    let supervisor3 = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    supervisor3.run().await.unwrap();
}

#[tokio::test]
async fn test_supervisor_foreground_with_lock() {
    let hooks = vec![create_test_hook(
        "echo",
        vec!["test".to_string()],
        false,
        false,
    )];
    let temp_dir = TempDir::new().unwrap();

    let supervisor =
        Supervisor::new_for_directory(temp_dir.path(), hooks, SupervisorMode::Foreground);

    // First supervisor should succeed
    assert!(supervisor.is_ok());
}

#[tokio::test]
#[ignore = "Flaky in CI - needs investigation"]
async fn test_supervisor_synchronous_mode() {
    let hooks = vec![create_test_hook(
        "echo",
        vec!["sync".to_string()],
        false,
        false,
    )];
    let supervisor = Supervisor::new(hooks, SupervisorMode::Synchronous).unwrap();

    let result = supervisor.run().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_dir_creation() {
    let cache_dir = get_cache_dir().unwrap();
    assert!(cache_dir.to_string_lossy().contains("cuenv"));
    assert!(cache_dir.to_string_lossy().contains("preload-cache"));
}

#[tokio::test]
async fn test_save_and_load_cached_environment() {
    let cache_dir = get_cache_dir().unwrap();
    fs::create_dir_all(&cache_dir).unwrap();

    let mut env_vars = HashMap::new();
    env_vars.insert("CACHED_VAR".to_string(), "cached_value".to_string());

    let captured = CapturedEnvironment {
        env_vars,
        input_hash: "test_save_load".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Save the environment
    let cache_file = cache_dir.join("test_save_load.json");
    let content = serde_json::to_string_pretty(&captured).unwrap();
    fs::write(&cache_file, content).unwrap();

    // Load it back
    let loaded_content = fs::read_to_string(&cache_file).unwrap();
    let loaded: CapturedEnvironment = serde_json::from_str(&loaded_content).unwrap();

    assert_eq!(loaded.input_hash, "test_save_load");
    assert_eq!(
        loaded.env_vars.get("CACHED_VAR"),
        Some(&"cached_value".to_string())
    );
}
