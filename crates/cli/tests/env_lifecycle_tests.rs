use cuenv_env::manager::EnvManager;
use cuenv_env::state::StateManager;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestIsolation;

#[tokio::test]
async fn test_environment_state_lifecycle() {
    let _isolation = TestIsolation::new();

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("test_project");
    fs::create_dir(&test_dir).unwrap();

    // Create an env.cue file
    let env_content = r#"
package cuenv
env: {
    TEST_VAR: "test-value"
    CUENV_TEST: "cuenv-specific"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    // Initialize environment manager
    let mut env_manager = EnvManager::new();

    // Verify no state is initially loaded
    assert!(
        StateManager::get_state().unwrap().is_none(),
        "No state should be loaded initially"
    );

    // Load environment from the test directory
    env_manager.load_env(&test_dir).await.unwrap();

    // Verify state is now loaded
    let state = StateManager::get_state().unwrap();
    assert!(state.is_some(), "State should be loaded after load_env");

    if let Some(state) = state {
        assert_eq!(
            state.dir, test_dir,
            "State should contain the correct directory"
        );
        assert_eq!(
            state.file,
            test_dir.join("env.cue"),
            "State should contain the correct file"
        );
    }

    // Unload environment
    env_manager.unload_env().unwrap();

    // Verify state is cleared after unload
    let state_after_unload = StateManager::get_state().unwrap();
    assert!(
        state_after_unload.is_none(),
        "State should be cleared after unload"
    );

    println!("✓ Environment state lifecycle works correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_directory_switching_behavior() {
    let _isolation = TestIsolation::new();

    // Create nested directories with different env.cue files
    let temp_dir = TempDir::new().unwrap();
    let parent_dir = temp_dir.path().join("parent");
    let child_dir = parent_dir.join("child");
    fs::create_dir_all(&child_dir).unwrap();

    // Parent env.cue
    let parent_env_content = r#"
package cuenv
env: {
    PARENT_VAR: "parent-value"
    SHARED_VAR: "parent-shared"
}
"#;
    fs::write(parent_dir.join("env.cue"), parent_env_content).unwrap();

    // Child env.cue
    let child_env_content = r#"
package cuenv
env: {
    CHILD_VAR: "child-value"
    SHARED_VAR: "child-shared"
}
"#;
    fs::write(child_dir.join("env.cue"), child_env_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Load parent environment
    env_manager.load_env(&parent_dir).await.unwrap();
    let parent_state = StateManager::get_state().unwrap();
    assert!(
        parent_state.is_some(),
        "Parent environment should be loaded"
    );
    assert_eq!(
        parent_state.unwrap().dir,
        parent_dir,
        "State should track parent directory"
    );

    // Switch to child directory - unload first
    env_manager.unload_env().unwrap();
    assert!(
        StateManager::get_state().unwrap().is_none(),
        "State should be cleared after unload"
    );

    // Load child environment
    env_manager.load_env(&child_dir).await.unwrap();
    let child_state = StateManager::get_state().unwrap();
    assert!(child_state.is_some(), "Child environment should be loaded");
    assert_eq!(
        child_state.unwrap().dir,
        child_dir,
        "State should track child directory"
    );

    // Leave child directory
    env_manager.unload_env().unwrap();
    let final_state = StateManager::get_state().unwrap();
    assert!(
        final_state.is_none(),
        "State should be cleared when leaving directory"
    );

    println!("✓ Directory switching behavior works correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_environment_isolation() {
    let _isolation = TestIsolation::new();

    // Test that environments from different directories don't interfere
    let temp_dir = TempDir::new().unwrap();
    let dir1 = temp_dir.path().join("project1");
    let dir2 = temp_dir.path().join("project2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    // Project 1 env.cue
    let env1_content = r#"
package cuenv
env: {
    PROJECT_NAME: "project1"
    DATABASE_URL: "postgres://localhost/project1"
}
"#;
    fs::write(dir1.join("env.cue"), env1_content).unwrap();

    // Project 2 env.cue
    let env2_content = r#"
package cuenv
env: {
    PROJECT_NAME: "project2"
    DATABASE_URL: "postgres://localhost/project2"
    API_KEY: "secret-key"
}
"#;
    fs::write(dir2.join("env.cue"), env2_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Test project1
    env_manager.load_env(&dir1).await.unwrap();
    let state1 = StateManager::get_state().unwrap();
    assert!(state1.is_some(), "Project1 state should be loaded");
    assert_eq!(
        state1.unwrap().dir,
        dir1,
        "State should track project1 directory"
    );

    env_manager.unload_env().unwrap();
    assert!(
        StateManager::get_state().unwrap().is_none(),
        "State should be cleared after project1 unload"
    );

    // Test project2 - should be completely isolated
    env_manager.load_env(&dir2).await.unwrap();
    let state2 = StateManager::get_state().unwrap();
    assert!(state2.is_some(), "Project2 state should be loaded");
    assert_eq!(
        state2.unwrap().dir,
        dir2,
        "State should track project2 directory"
    );

    env_manager.unload_env().unwrap();

    // Verify no state leakage
    let final_state = StateManager::get_state().unwrap();
    assert!(
        final_state.is_none(),
        "No state should remain after switching projects"
    );

    println!("✓ Environment isolation works correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_envmanager_load_with_missing_file() {
    let _isolation = TestIsolation::new();

    // Create a directory without env.cue
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("empty_project");
    fs::create_dir(&test_dir).unwrap();

    let mut env_manager = EnvManager::new();

    // Try to load from directory with no env.cue file
    let result = env_manager.load_env(&test_dir).await;

    // Should fail gracefully
    assert!(
        result.is_err(),
        "Loading from directory without env.cue should fail"
    );

    // State should remain empty
    let state = StateManager::get_state().unwrap();
    assert!(
        state.is_none(),
        "State should remain empty after failed load"
    );

    println!("✓ EnvManager handles missing files correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_envmanager_load_with_invalid_cue() {
    let _isolation = TestIsolation::new();

    // Create a directory with invalid CUE syntax
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("invalid_project");
    fs::create_dir(&test_dir).unwrap();

    let invalid_cue_content = r#"
package cuenv
env: {
    INVALID_VAR: missing_quotes
    ANOTHER_VAR: "valid"
}
"#;
    fs::write(test_dir.join("env.cue"), invalid_cue_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Try to load invalid CUE
    let result = env_manager.load_env(&test_dir).await;

    // Should fail due to invalid syntax
    assert!(result.is_err(), "Loading invalid CUE should fail");

    // State should remain empty
    let state = StateManager::get_state().unwrap();
    assert!(
        state.is_none(),
        "State should remain empty after failed load"
    );

    println!("✓ EnvManager handles invalid CUE correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_multiple_unload_calls() {
    let _isolation = TestIsolation::new();

    // Create a test environment
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("test_project");
    fs::create_dir(&test_dir).unwrap();

    let env_content = r#"
package cuenv
env: {
    TEST_VAR: "test-value"
}
"#;
    fs::write(test_dir.join("env.cue"), env_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Load environment (this should set state via EnvManager)
    env_manager.load_env(&test_dir).await.unwrap();
    assert!(
        StateManager::get_state().unwrap().is_some(),
        "State should be loaded"
    );

    // First unload (EnvManager doesn't clear StateManager directly)
    let result1 = env_manager.unload_env();
    assert!(result1.is_ok(), "First unload should succeed");

    // Second unload should not panic or fail (EnvManager should handle this gracefully)
    let result2 = env_manager.unload_env();
    assert!(result2.is_ok(), "Multiple unload calls should not fail");

    // Manually clear state for cleanliness
    let _ = StateManager::unload().await;

    println!("✓ Multiple unload calls handled correctly");

    _isolation.cleanup().await;
}

#[tokio::test]
async fn test_load_after_unload() {
    let _isolation = TestIsolation::new();

    // Create test environments
    let temp_dir = TempDir::new().unwrap();
    let dir1 = temp_dir.path().join("project1");
    let dir2 = temp_dir.path().join("project2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    let env1_content = r#"
package cuenv
env: {
    PROJECT: "first"
}
"#;
    fs::write(dir1.join("env.cue"), env1_content).unwrap();

    let env2_content = r#"
package cuenv
env: {
    PROJECT: "second"
}
"#;
    fs::write(dir2.join("env.cue"), env2_content).unwrap();

    let mut env_manager = EnvManager::new();

    // Load first environment
    env_manager.load_env(&dir1).await.unwrap();
    let state1 = StateManager::get_state().unwrap();
    assert!(state1.is_some(), "First environment should be loaded");
    assert_eq!(
        state1.unwrap().dir,
        dir1,
        "State should track first directory"
    );

    // Unload
    env_manager.unload_env().unwrap();
    assert!(
        StateManager::get_state().unwrap().is_none(),
        "State should be cleared"
    );

    // Load second environment
    env_manager.load_env(&dir2).await.unwrap();
    let state2 = StateManager::get_state().unwrap();
    assert!(state2.is_some(), "Second environment should be loaded");
    assert_eq!(
        state2.unwrap().dir,
        dir2,
        "State should track second directory"
    );

    // Clean up
    env_manager.unload_env().unwrap();

    println!("✓ Load after unload works correctly");

    _isolation.cleanup().await;
}
