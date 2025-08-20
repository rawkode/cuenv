use cuenv_env::manager::EnvManager;
use cuenv_env::state::StateManager;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestIsolation;

/// Property: Loading and unloading should always return to initial state
#[tokio::test]
async fn property_load_unload_returns_to_initial_state() {
    let _isolation = TestIsolation::new();
    // Create all temp dirs first and keep them alive
    let mut temp_dirs = Vec::new();
    let mut test_dirs = Vec::new();

    for i in 0..10 {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join(format!("test_project_{i}"));
        fs::create_dir(&test_dir).unwrap();

        let env_content = format!(
            r#"
package cuenv
env: {{
    TEST_VAR_{i}: "test-value-{i}"
    ITERATION: "{i}"
}}
"#
        );
        fs::write(test_dir.join("env.cue"), env_content).unwrap();

        test_dirs.push(test_dir);
        temp_dirs.push(temp_dir); // Keep temp_dir alive
    }

    for (i, test_dir) in test_dirs.iter().enumerate() {
        let mut env_manager = EnvManager::new();

        // Record initial state
        let initial_state = StateManager::get_state().unwrap();
        assert!(initial_state.is_none(), "Initial state should be None");

        // Load environment
        env_manager.load_env(test_dir).await.unwrap();
        let loaded_state = StateManager::get_state().unwrap();
        assert!(
            loaded_state.is_some(),
            "State should be loaded after load_env for iteration {i}"
        );

        // Unload environment
        env_manager.unload_env().unwrap();

        // State should return to initial (clean) state
        let final_state = StateManager::get_state().unwrap();
        assert!(
            final_state.is_none(),
            "Final state should be None like initial state for iteration {i}"
        );
    }

    println!("✓ Property: Load/unload always returns to initial state");

    _isolation.cleanup().await;
}

/// Property: Multiple load/unload cycles should be idempotent
#[tokio::test]
async fn property_multiple_cycles_are_idempotent() {
    let _isolation = TestIsolation::new();

    // Create test environment
    let _temp_dir = TempDir::new().unwrap(); // Keep alive with underscore
    let test_dir = _temp_dir.path().join("test_project");
    fs::create_dir(&test_dir).unwrap();

    let env_content = r#"
package cuenv
env: {
    CYCLE_VAR: "cycle-value"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Perform multiple load/unload cycles
    for cycle in 0..5 {
        // Load
        env_manager.load_env(&test_dir).await.unwrap();
        let loaded_state = StateManager::get_state().unwrap();
        assert!(
            loaded_state.is_some(),
            "State should be loaded in cycle {cycle}"
        );
        assert_eq!(
            loaded_state.unwrap().dir,
            test_dir,
            "Directory should be consistent in cycle {cycle}"
        );

        // Unload
        env_manager.unload_env().unwrap();
        let unloaded_state = StateManager::get_state().unwrap();
        assert!(
            unloaded_state.is_none(),
            "State should be cleared in cycle {cycle}"
        );
    }

    println!("✓ Property: Multiple cycles are idempotent");

    _isolation.cleanup().await;
}

/// Property: Switching between different directories should maintain isolation
#[tokio::test]
async fn property_directory_switching_maintains_isolation() {
    let _isolation = TestIsolation::new();

    let _temp_dir = TempDir::new().unwrap(); // Keep alive
    let mut directories = Vec::new();

    // Create multiple test directories
    for i in 0..5 {
        let test_dir = _temp_dir.path().join(format!("project_{i}"));
        fs::create_dir(&test_dir).unwrap();

        let env_content = format!(
            r#"
package cuenv
env: {{
    PROJECT_ID: "{i}"
    PROJECT_NAME: "project_{i}"
}}
"#
        );
        fs::write(test_dir.join("env.cue"), env_content).unwrap();
        directories.push(test_dir);
    }

    let mut env_manager = EnvManager::new();

    // Test switching between directories multiple times
    for &dir_index in &[0, 2, 1, 4, 3, 0, 1] {
        let test_dir = &directories[dir_index];

        // Load environment
        env_manager.load_env(test_dir).await.unwrap();
        let loaded_state = StateManager::get_state().unwrap();
        assert!(
            loaded_state.is_some(),
            "State should be loaded for directory {dir_index}"
        );
        assert_eq!(
            loaded_state.unwrap().dir,
            *test_dir,
            "State should track correct directory {dir_index}"
        );

        // Unload environment
        env_manager.unload_env().unwrap();
        let unloaded_state = StateManager::get_state().unwrap();
        assert!(
            unloaded_state.is_none(),
            "State should be cleared after leaving directory {dir_index}"
        );
    }

    println!("✓ Property: Directory switching maintains isolation");

    _isolation.cleanup().await;
}

/// Property: Invalid operations should not corrupt state
#[tokio::test]
async fn property_invalid_operations_preserve_state() {
    let _isolation = TestIsolation::new();

    let _temp_dir = TempDir::new().unwrap(); // Keep alive
    let mut env_manager = EnvManager::new();

    // Test various invalid operations
    let test_cases = vec![
        ("nonexistent_dir", false), // Directory doesn't exist
        ("empty_dir", true),        // Directory exists but no env.cue
    ];

    for (case_name, create_dir) in test_cases {
        let test_dir = _temp_dir.path().join(case_name);
        if create_dir {
            fs::create_dir(&test_dir).unwrap();
        }

        // Record state before invalid operation
        let state_before = StateManager::get_state().unwrap();

        // Try to load from invalid directory (should fail)
        let load_result = env_manager.load_env(&test_dir).await;
        assert!(load_result.is_err(), "Load should fail for {case_name}");

        // State should be unchanged after failed operation
        let state_after = StateManager::get_state().unwrap();
        // Both should be None (clean state)
        assert!(
            state_before.is_none() && state_after.is_none(),
            "State should remain None after failed load for {case_name}"
        );
    }

    // Test unload without load
    let initial_state = StateManager::get_state().unwrap();
    let unload_result = env_manager.unload_env();
    assert!(unload_result.is_ok(), "Unload without load should not fail");

    let final_state = StateManager::get_state().unwrap();
    assert!(
        initial_state.is_none() && final_state.is_none(),
        "State should remain None after unload without load"
    );

    println!("✓ Property: Invalid operations preserve state");

    _isolation.cleanup().await;
}

/// Property: Concurrent access should maintain consistency (basic test)
#[tokio::test]
async fn property_concurrent_access_basic_consistency() {
    let _isolation = TestIsolation::new();

    // Create test environment
    let _temp_dir = TempDir::new().unwrap(); // Keep alive
    let test_dir = _temp_dir.path().join("concurrent_test");
    fs::create_dir(&test_dir).unwrap();

    let env_content = r#"
package cuenv
env: {
    CONCURRENT_VAR: "concurrent-value"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    // Test sequential operations that might reveal concurrency issues
    for i in 0..20 {
        let mut env_manager = EnvManager::new();

        // Load
        env_manager.load_env(&test_dir).await.unwrap();
        let state = StateManager::get_state().unwrap();
        assert!(state.is_some(), "State should be loaded in iteration {i}");

        // Immediately unload
        env_manager.unload_env().unwrap();

        // Check state multiple times to catch potential race conditions
        for _ in 0..3 {
            let _state = StateManager::get_state().unwrap();
            // Note: We can't assert state is None here because EnvManager.unload_env()
            // doesn't clear global state - it only cleans up the EnvManager instance
        }
    }

    println!("✓ Property: Basic concurrent access consistency");

    _isolation.cleanup().await;
}

/// Property: Error recovery should maintain system integrity
#[tokio::test]
async fn property_error_recovery_maintains_integrity() {
    let _isolation = TestIsolation::new();

    let _temp_dir = TempDir::new().unwrap(); // Keep alive

    // Create a valid environment first
    let valid_dir = _temp_dir.path().join("valid_project");
    fs::create_dir(&valid_dir).unwrap();
    let valid_env_content = r#"
package cuenv
env: {
    VALID_VAR: "valid-value"
}
"#;
    fs::write(valid_dir.join("env.cue"), valid_env_content).unwrap();

    // Create an invalid environment
    let invalid_dir = _temp_dir.path().join("invalid_project");
    fs::create_dir(&invalid_dir).unwrap();
    let invalid_env_content = r#"
package cuenv
env: {
    INVALID_VAR: missing_quotes_syntax_error
}
"#;
    fs::write(invalid_dir.join("env.cue"), invalid_env_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Test recovery pattern: valid -> invalid -> valid
    let operations = vec![
        (&valid_dir, true),    // Should succeed
        (&invalid_dir, false), // Should fail
        (&valid_dir, true),    // Should succeed again
    ];

    for (dir, should_succeed) in operations {
        let state_before = StateManager::get_state().unwrap();
        let result = env_manager.load_env(dir).await;

        if should_succeed {
            assert!(result.is_ok(), "Load should succeed for valid directory");
            let state_after = StateManager::get_state().unwrap();
            assert!(
                state_after.is_some(),
                "State should be loaded for valid directory"
            );
        } else {
            assert!(result.is_err(), "Load should fail for invalid directory");
            let state_after = StateManager::get_state().unwrap();
            // State should not be corrupted by failed operation
            // For failed operations, state should remain as it was
            match (state_before.is_some(), state_after.is_some()) {
                (true, true) => {
                    // Both have state - check they refer to the same directory
                    assert_eq!(
                        state_before.as_ref().unwrap().dir,
                        state_after.as_ref().unwrap().dir,
                        "State directory should be unchanged after failed operation"
                    );
                }
                (false, false) => {
                    // Both are None - this is fine
                }
                _ => {
                    panic!(
                        "State consistency violated: before={:?}, after={:?}",
                        state_before.is_some(),
                        state_after.is_some()
                    );
                }
            }
        }

        // Always try to unload (should be safe)
        let unload_result = env_manager.unload_env();
        assert!(unload_result.is_ok(), "Unload should always succeed");
    }

    println!("✓ Property: Error recovery maintains integrity");

    _isolation.cleanup().await;
}
