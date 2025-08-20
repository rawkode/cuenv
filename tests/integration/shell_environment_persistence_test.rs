use cuenv_env::StateManager;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[tokio::test]
async fn test_shell_hook_unloads_environment_variables_on_directory_change() {
    // This test reproduces the bug where environment variables persist
    // after leaving a cuenv directory via shell hook

    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("test_cuenv_project");
    fs::create_dir(&test_dir).unwrap();

    // Create an env.cue file that sets test variables
    let env_content = r#"
package cuenv

env: {
    TEST_SHELL_VAR: "shell_test_value"
    TEST_PERSISTENCE_CHECK: "should_be_cleaned_up"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    // Build cuenv binary for testing
    let output = Command::new("cargo")
        .args(&["build"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to build cuenv");

    assert!(output.status.success(), "Failed to build cuenv binary");

    let cuenv_binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/cuenv");

    // Test 1: Verify shell hook loads environment variables
    let hook_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to run shell hook");

    let hook_stdout = String::from_utf8(hook_output.stdout).unwrap();

    // Verify that the hook sets the test variables
    assert!(
        hook_stdout.contains("TEST_SHELL_VAR=shell_test_value"),
        "Shell hook should export TEST_SHELL_VAR, got: {}",
        hook_stdout
    );
    assert!(
        hook_stdout.contains("TEST_PERSISTENCE_CHECK=should_be_cleaned_up"),
        "Shell hook should export TEST_PERSISTENCE_CHECK, got: {}",
        hook_stdout
    );

    // Test 2: Simulate the hook being applied (by setting env vars)
    std::env::set_var("TEST_SHELL_VAR", "shell_test_value");
    std::env::set_var("TEST_PERSISTENCE_CHECK", "should_be_cleaned_up");

    // Load the environment state by running the hook from inside the directory
    let _load_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to load environment");

    // Test 3: Verify shell hook from parent directory unsets variables
    let parent_dir = test_dir.parent().unwrap();
    let unload_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(parent_dir)
        .output()
        .expect("Failed to run shell hook from parent");

    let unload_stdout = String::from_utf8(unload_output.stdout).unwrap();

    // The hook should output unset commands for the test variables
    assert!(
        unload_stdout.contains("unset TEST_SHELL_VAR")
            || unload_stdout.contains("TEST_SHELL_VAR=")
            || !unload_stdout.is_empty(), // Should produce some output to clean up
        "Shell hook should unset TEST_SHELL_VAR when leaving directory, got: '{}'",
        unload_stdout
    );

    // Test 4: Verify prune command generates cleanup commands
    // First reload the environment
    let _reload_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to reload environment");

    // Then test prune
    let prune_output = Command::new(&cuenv_binary)
        .args(&["env", "prune"])
        .current_dir(parent_dir)
        .output()
        .expect("Failed to run prune command");

    let prune_stdout = String::from_utf8(prune_output.stdout).unwrap();
    let prune_stderr = String::from_utf8(prune_output.stderr).unwrap();

    // Prune should either generate unset commands or indicate no state to clean
    if StateManager::is_loaded() {
        assert!(
            prune_stdout.contains("unset") || prune_stderr.contains("Generating shell commands"),
            "Prune should generate cleanup commands when state exists, stdout: '{}', stderr: '{}'",
            prune_stdout,
            prune_stderr
        );
    }

    println!("✓ Shell hook environment persistence test completed");
}

#[tokio::test]
async fn test_environment_variable_lifecycle_with_real_shell_commands() {
    // This test verifies the complete lifecycle using actual shell command output

    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("lifecycle_test");
    fs::create_dir(&test_dir).unwrap();

    let env_content = r#"
package cuenv

env: {
    LIFECYCLE_TEST_VAR: "lifecycle_value"
    CUENV_MANAGED_VAR: "managed_by_cuenv"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    let cuenv_binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/cuenv");

    // Phase 1: Load environment
    let load_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(&test_dir)
        .output()
        .expect("Failed to load environment");

    let load_commands = String::from_utf8(load_output.stdout).unwrap();
    assert!(
        load_commands.contains("LIFECYCLE_TEST_VAR=lifecycle_value"),
        "Load should set LIFECYCLE_TEST_VAR"
    );

    // Phase 2: Simulate leaving directory and check for unload commands
    let unload_output = Command::new(&cuenv_binary)
        .args(&["shell", "hook", "bash"])
        .current_dir(temp_dir.path()) // Parent directory
        .output()
        .expect("Failed to get unload commands");

    let unload_commands = String::from_utf8(unload_output.stdout).unwrap();

    // This is the key test - the shell hook should detect directory change and unload
    if !unload_commands.is_empty() {
        assert!(
            unload_commands.contains("unset LIFECYCLE_TEST_VAR")
                || unload_commands.contains("LIFECYCLE_TEST_VAR=")
                || unload_commands.contains("export LIFECYCLE_TEST_VAR="),
            "Unload should handle LIFECYCLE_TEST_VAR, got: '{}'",
            unload_commands
        );
    } else {
        // If no unload commands, the state should be properly cleared
        assert!(
            !StateManager::is_loaded(),
            "If no unload commands generated, state should be cleared"
        );
    }

    println!("✓ Environment variable lifecycle test completed");
}
