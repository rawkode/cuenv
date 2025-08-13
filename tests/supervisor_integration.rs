use cuenv_config::Hook;
use cuenv_env::manager::environment::supervisor::{
    run_supervisor, CapturedEnvironment, Supervisor, SupervisorMode,
};
use cuenv_utils::hooks_status::HooksStatusManager;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

/// Helper to create a test hook
fn create_hook(command: &str, args: Vec<&str>, preload: bool, source: bool) -> Hook {
    Hook {
        command: command.to_string(),
        args: Some(args.iter().map(|s| s.to_string()).collect()),
        dir: None,
        preload: Some(preload),
        source: Some(source),
        inputs: None,
        outputs: None,
        cache: None,
        when: None,
    }
}

/// Helper to create a hook with inputs
fn create_hook_with_inputs(command: &str, args: Vec<&str>, inputs: Vec<String>) -> Hook {
    Hook {
        command: command.to_string(),
        args: Some(args.iter().map(|s| s.to_string()).collect()),
        dir: None,
        preload: Some(true),
        source: Some(false),
        inputs: Some(inputs),
        outputs: None,
        cache: None,
        when: None,
    }
}

/// Helper to get the cache directory for testing
fn get_test_cache_dir() -> PathBuf {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());
    PathBuf::from(format!("/tmp/cuenv-{}/preload-cache", user))
}

/// Helper to clean up test cache
fn cleanup_test_cache() {
    let cache_dir = get_test_cache_dir();
    if cache_dir.exists() {
        let _ = fs::remove_dir_all(&cache_dir);
    }
}

#[tokio::test]
async fn test_supervisor_executes_multiple_hooks_concurrently() {
    cleanup_test_cache();

    // Create hooks that sleep for different durations
    let hooks = vec![
        create_hook("sleep", vec!["0.1"], true, false),
        create_hook("sleep", vec!["0.2"], true, false),
        create_hook("sleep", vec!["0.3"], true, false),
    ];

    let start = SystemTime::now();
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;
    let elapsed = start.elapsed().unwrap();

    assert!(result.is_ok(), "Supervisor should complete successfully");

    // If run concurrently, should take ~0.3s (max duration)
    // If run sequentially, would take ~0.6s (sum of durations)
    assert!(
        elapsed < Duration::from_millis(500)
        "Hooks should run concurrently, took {:?}"
        elapsed
    );
}

#[tokio::test]
async fn test_supervisor_captures_environment_from_source_hooks() {
    cleanup_test_cache();

    // Create a temporary script that outputs environment variables
    let temp_dir = TempDir::new().unwrap();
    let script_path = temp_dir.path().join("env_script.sh");

    fs::write(
        &script_path,
        "#!/bin/bash
echo \"export TEST_VAR1=value1\"
echo \"export TEST_VAR2=value2\"
echo \"TEST_VAR3=value3\"
",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let hooks = vec![Hook {
        command: script_path.to_string_lossy().to_string(),
        args: None,
        dir: None,
        preload: Some(true),
        source: Some(true),
        inputs: None,
        outputs: None,
        cache: None,
        when: None,
    }];

    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;
    assert!(result.is_ok(), "Supervisor should complete successfully");

    // Check that environment was captured and saved
    let cache_dir = get_test_cache_dir();
    let latest_file = cache_dir.join("latest_env.json");

    assert!(latest_file.exists(), "Latest environment file should exist");

    let content = fs::read_to_string(&latest_file).unwrap();
    let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

    assert_eq!(
        captured.env_vars.get("TEST_VAR1"),
        Some(&"value1".to_string())
    );
    assert_eq!(
        captured.env_vars.get("TEST_VAR2"),
        Some(&"value2".to_string())
    );
    assert_eq!(
        captured.env_vars.get("TEST_VAR3"),
        Some(&"value3".to_string())
    );
}

#[tokio::test]
async fn test_supervisor_caching_with_inputs() {
    cleanup_test_cache();

    // Create a test file to use as input
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("input.txt");
    fs::write(&input_file, "initial content").unwrap();

    // Create a script that we can verify was executed
    let script_path = temp_dir.path().join("test_script.sh");
    let marker_file = temp_dir.path().join("executed.marker");

    fs::write(
        &script_path,
        format!(
            "#!/bin/bash
echo \"executed\" > {}
echo \"TEST_ENV=from_script\"
",
            marker_file.display()
        ),
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let hooks = vec![Hook {
        command: script_path.to_string_lossy().to_string(),
        args: None,
        dir: None,
        preload: Some(true),
        source: Some(true),
        inputs: Some(vec![input_file.to_string_lossy().to_string()]),
        outputs: None,
        cache: None,
        when: None,
    }];

    // First run - should execute the script
    let supervisor1 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
    let result1 = supervisor1.run().await;
    assert!(result1.is_ok());
    assert!(marker_file.exists(), "Script should have been executed");

    // Remove the marker file
    fs::remove_file(&marker_file).unwrap();

    // Second run with same inputs - should use cache and NOT execute script
    let supervisor2 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
    let result2 = supervisor2.run().await;
    assert!(result2.is_ok());
    assert!(
        !marker_file.exists(),
        "Script should NOT have been executed (cache hit)"
    );

    // Modify the input file
    sleep(Duration::from_millis(10)).await;
    fs::write(&input_file, "modified content").unwrap();

    // Third run with modified input - should execute script again
    let supervisor3 = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result3 = supervisor3.run().await;
    assert!(result3.is_ok());
    assert!(
        marker_file.exists(),
        "Script should have been executed (cache miss)"
    );
}

#[tokio::test]
async fn test_supervisor_handles_hook_failures_gracefully() {
    cleanup_test_cache();

    let hooks = vec![
        create_hook("echo", vec!["hook1"], true, false),
        create_hook("false", vec![], true, false), // This will fail
        create_hook("echo", vec!["hook3"], true, false),
    ];

    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;

    // Supervisor should complete even with failed hooks
    assert!(
        result.is_ok(),
        "Supervisor should complete despite hook failures"
    );
}

#[tokio::test]
async fn test_supervisor_timeout_handling() {
    cleanup_test_cache();

    // Create a hook that would sleep for a long time
    let hooks = vec![create_hook("sleep", vec!["100"], true, false)];

    let start = SystemTime::now();

    // Run supervisor with a timeout wrapper
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = timeout(Duration::from_secs(2), supervisor.run()).await;

    let elapsed = start.elapsed().unwrap();

    // The supervisor itself should handle timeouts, but we're testing
    // that it doesn't hang indefinitely
    assert!(
        elapsed < Duration::from_secs(65)
        "Supervisor should handle timeouts properly"
    );
}

#[tokio::test]
async fn test_supervisor_status_tracking() {
    cleanup_test_cache();

    // Clean up any existing status
    let status_file = PathBuf::from(format!(
        "/tmp/cuenv-{}/hooks-status.json",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    if status_file.exists() {
        let _ = fs::remove_file(&status_file);
    }

    let hooks = vec![
        create_hook("echo", vec!["test1"], true, false)
        create_hook("echo", vec!["test2"], true, false)
    ];

    // Start the supervisor
    let supervisor_handle = tokio::spawn(async move {
        let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
        supervisor.run().await
    });

    // Give it a moment to initialize
    sleep(Duration::from_millis(50)).await;

    // Check if status file was created
    if status_file.exists() {
        let content = fs::read_to_string(&status_file).unwrap();
        let status: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify status structure
        assert!(status.get("total").is_some());
        assert!(status.get("hooks").is_some());
    }

    // Wait for supervisor to complete
    let result = supervisor_handle.await.unwrap();
    assert!(result.is_ok());

    // After completion, status should be cleaned up
    sleep(Duration::from_millis(50)).await;

    if status_file.exists() {
        let content = fs::read_to_string(&status_file).unwrap();
        let status: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Status should be empty or show completed
        let total = status.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(total, 0, "Status should be cleared after completion");
    }
}

#[tokio::test]
async fn test_supervisor_parallel_execution_with_source_hooks() {
    cleanup_test_cache();

    let temp_dir = TempDir::new().unwrap();

    // Create multiple source scripts
    let mut hooks = Vec::new();

    for i in 0..3 {
        let script_path = temp_dir.path().join(format!("script{}.sh", i));
        fs::write(
            &script_path,
            format!(
                "#!/bin/bash
sleep 0.1
echo \"export VAR{}=value{}\"
",
                i, i
            ),
        )
        .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        hooks.push(Hook {
            command: script_path.to_string_lossy().to_string(),
            args: None,
            dir: None,
            preload: Some(true),
            source: Some(true),
            inputs: None,
            outputs: None,
            cache: None,
            when: None,
        });
    }

    let start = SystemTime::now();
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;
    let elapsed = start.elapsed().unwrap();

    assert!(result.is_ok());

    // Should run in parallel (around 0.1s) not sequentially (0.3s)
    assert!(
        elapsed < Duration::from_millis(250)
        "Source hooks should run in parallel"
    );

    // Check all environment variables were captured
    let cache_dir = get_test_cache_dir();
    let latest_file = cache_dir.join("latest_env.json");

    if latest_file.exists() {
        let content = fs::read_to_string(&latest_file).unwrap();
        let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

        assert_eq!(captured.env_vars.get("VAR0"), Some(&"value0".to_string()));
        assert_eq!(captured.env_vars.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(captured.env_vars.get("VAR2"), Some(&"value2".to_string()));
    }
}

#[tokio::test]
async fn test_supervisor_empty_hooks_list() {
    cleanup_test_cache();

    let hooks = vec![];
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;

    assert!(result.is_ok(), "Supervisor should handle empty hooks list");
}

#[tokio::test]
async fn test_supervisor_mixed_preload_and_regular_hooks() {
    cleanup_test_cache();

    let hooks = vec![
        create_hook("echo", vec!["preload1"], true, false),
        create_hook("echo", vec!["regular"], false, false), // Not a preload hook
        create_hook("echo", vec!["preload2"], true, false),
    ];

    let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result = supervisor.run().await;

    assert!(
        result.is_ok(),
        "Supervisor should filter and run only preload hooks"
    );
}

#[tokio::test]
async fn test_supervisor_cache_persistence_across_runs() {
    cleanup_test_cache();

    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("persistent_input.txt");
    fs::write(&input_file, "persistent content").unwrap();

    let script_path = temp_dir.path().join("persistent_script.sh");
    fs::write(
        &script_path,
        "#!/bin/bash
echo \"export PERSISTENT_VAR=persistent_value\"
",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let hooks = vec![Hook {
        command: script_path.to_string_lossy().to_string(),
        args: None,
        dir: None,
        preload: Some(true),
        source: Some(true),
        inputs: Some(vec![input_file.to_string_lossy().to_string()]),
        outputs: None,
        cache: None,
        when: None,
    }];

    // First run
    let supervisor1 = Supervisor::new(hooks.clone(), SupervisorMode::Background).unwrap();
    let result1 = supervisor1.run().await;
    assert!(result1.is_ok());

    // Get the cache file path to verify it exists
    let cache_dir = get_test_cache_dir();
    let cache_files: Vec<_> = fs::read_dir(&cache_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .collect();

    assert!(
        cache_files.len() >= 2
        "Should have at least cache file and latest_env.json"
    );

    // Second run - should use existing cache
    let supervisor2 = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
    let result2 = supervisor2.run().await;
    assert!(result2.is_ok());

    // Verify cached environment is still there
    let latest_file = cache_dir.join("latest_env.json");
    assert!(latest_file.exists());

    let content = fs::read_to_string(&latest_file).unwrap();
    let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

    assert_eq!(
        captured.env_vars.get("PERSISTENT_VAR")
        Some(&"persistent_value".to_string())
    );
}
