use cuenv_config::{Hook, ParseOptions};
use cuenv_env::EnvManager;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[tokio::test]
async fn test_preload_hooks_run_in_background() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with preload hooks
    fs::write(
        &env_file,
        r#"package cuenv
env: {
    TEST_VAR: "test"
}
hooks: onEnter: [
    // Regular hook - should block
    {
        command: "echo"
        args: ["regular hook"]
    },
    // Preload hook - should not block
    {
        command: "sleep"
        args: ["2"]
        preload: true
    },
]"#,
    )
    .unwrap();

    let mut env_manager = EnvManager::new();
    let start = Instant::now();

    // Load environment - should return quickly despite sleep command
    env_manager
        .load_env_with_options(temp_dir.path(), None, Vec::new(), None)
        .await
        .unwrap();

    let load_time = start.elapsed();

    // Loading should be quick (not waiting for 2 second sleep)
    assert!(
        load_time < Duration::from_secs(1),
        "Environment loading took too long: {:?}",
        load_time
    );

    // Now wait for preload hooks
    let wait_start = Instant::now();
    env_manager.wait_for_preload_hooks().await.unwrap();
    let wait_time = wait_start.elapsed();

    // Waiting should take approximately 2 seconds (the sleep duration)
    assert!(
        wait_time >= Duration::from_millis(1900) && wait_time < Duration::from_secs(3),
        "Wait time was unexpected: {:?}",
        wait_time
    );
}

#[tokio::test]
async fn test_preload_hooks_with_source_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with mixed hook types
    fs::write(
        &env_file,
        r#"package cuenv
env: {
    BASE_VAR: "base"
}
hooks: onEnter: [
    // Source hook - must run synchronously
    {
        command: "echo"
        args: ["export SOURCED_VAR=from_hook"]
        source: true
    },
    // Preload hook - runs in background
    {
        command: "echo"
        args: ["preload task"]
        preload: true
    },
    // Regular hook - runs synchronously
    {
        command: "echo"
        args: ["regular task"]
    },
]"#,
    )
    .unwrap();

    let mut env_manager = EnvManager::new();

    // Load environment
    env_manager
        .load_env_with_options(temp_dir.path(), None, Vec::new(), None)
        .await
        .unwrap();

    // Check that CUE variables are available immediately
    let cue_vars = env_manager.get_cue_vars();
    assert_eq!(cue_vars.get("BASE_VAR"), Some(&"base".to_string()));

    // Wait for all hooks to complete
    env_manager.wait_for_preload_hooks().await.unwrap();
}

#[tokio::test]
async fn test_preload_hooks_cancellation() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with a long-running preload hook
    fs::write(
        &env_file,
        r#"package cuenv
env: {}
hooks: onEnter: [
    {
        command: "sleep"
        args: ["10"]  // Long sleep
        preload: true
    },
]"#,
    )
    .unwrap();

    let mut env_manager = EnvManager::new();

    // Load environment
    env_manager
        .load_env_with_options(temp_dir.path(), None, Vec::new(), None)
        .await
        .unwrap();

    // Cancel preload hooks immediately
    let start = Instant::now();
    env_manager.cancel_preload_hooks().await;
    let cancel_time = start.elapsed();

    // Cancellation should be quick
    assert!(
        cancel_time < Duration::from_millis(500),
        "Cancellation took too long: {:?}",
        cancel_time
    );
}

#[tokio::test]
async fn test_preload_hooks_status() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with multiple preload hooks
    fs::write(
        &env_file,
        r#"package cuenv
env: {}
hooks: onEnter: [
    {
        command: "sleep"
        args: ["1"]
        preload: true
    },
    {
        command: "echo"
        args: ["task2"]
        preload: true
    },
]"#,
    )
    .unwrap();

    let mut env_manager = EnvManager::new();

    // Load environment
    env_manager
        .load_env_with_options(temp_dir.path(), None, Vec::new(), None)
        .await
        .unwrap();

    // Check status immediately - should have running hooks
    let status = env_manager.get_preload_hooks_status().await;
    assert!(!status.is_empty(), "Should have running preload hooks");

    // Wait for completion
    env_manager.wait_for_preload_hooks().await.unwrap();

    // Check status after completion - should be empty
    let status_after = env_manager.get_preload_hooks_status().await;
    assert!(
        status_after.is_empty(),
        "Should have no running hooks after completion"
    );
}

#[test]
fn test_hook_preload_field_parsing() {
    // Test that the preload field is correctly parsed from CUE
    let cue_content = r#"{
        "hooks": {
            "onEnter": [
                {
                    "command": "echo",
                    "args": ["test"],
                    "preload": true
                },
                {
                    "command": "echo",
                    "args": ["test2"]
                }
            ]
        }
    }"#;

    let parsed: HashMap<String, Vec<Hook>> = serde_json::from_str(
        &serde_json::from_str::<serde_json::Value>(cue_content).unwrap()["hooks"].to_string(),
    )
    .unwrap();

    let on_enter_hooks = parsed.get("onEnter").unwrap();
    assert_eq!(on_enter_hooks.len(), 2);
    assert_eq!(on_enter_hooks[0].preload, Some(true));
    assert_eq!(on_enter_hooks[1].preload, None);
}

#[tokio::test]
async fn test_exec_command_waits_for_preload_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with a preload hook that sets an environment variable
    fs::write(
        &env_file,
        r#"package cuenv
import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

hooks: onEnter: [
    {
        command: "bash"
        args: ["-c", """
            sleep 2
            echo 'export PRELOAD_TEST_VAR="hook_completed"'
            echo 'export HOOK_TIMESTAMP="'$(date +%s)'"'
            """]
        source: true
        preload: true
    }
]

env: {
    BASE_VAR: "base_value"
}"#,
    )
    .unwrap();

    // Build the cuenv binary path
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let cuenv_path = std::path::Path::new(&manifest_dir)
        .join("target")
        .join("debug")
        .join("cuenv");

    if !cuenv_path.exists() {
        panic!(
            "cuenv binary not found at {:?}. Please run 'cargo build' first.",
            cuenv_path
        );
    }

    // Execute cuenv exec printenv in the temp directory
    let start_time = Instant::now();
    let output = std::process::Command::new(cuenv_path)
        .args(&["exec", "printenv"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute cuenv exec");
    let duration = start_time.elapsed();

    // Check that the command completed successfully
    assert!(output.status.success(), "cuenv exec command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);
    println!("Duration: {:?}", duration);

    // Verify that the preload hook environment variable is set
    assert!(
        stdout.contains("PRELOAD_TEST_VAR=hook_completed"),
        "Expected PRELOAD_TEST_VAR to be set by preload hook.\nSTDOUT: {}",
        stdout
    );

    // Verify that the base environment variable is also set
    assert!(
        stdout.contains("BASE_VAR=base_value"),
        "Expected BASE_VAR to be set from CUE config.\nSTDOUT: {}",
        stdout
    );

    // Verify that the hook timestamp is set
    assert!(
        stdout.contains("HOOK_TIMESTAMP="),
        "Expected HOOK_TIMESTAMP to be set by preload hook.\nSTDOUT: {}",
        stdout
    );

    // Command should complete in reasonable time (including the 2 second sleep + some overhead)
    assert!(
        duration >= Duration::from_secs(2) && duration < Duration::from_secs(10),
        "Command should take at least 2 seconds (for sleep) but complete within 10 seconds. Took: {:?}",
        duration
    );
}
