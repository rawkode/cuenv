use cuenv_env::manager::EnvManager;
use cuenv_env::state::StateManager;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[tokio::test]
async fn test_environment_loads_and_unloads_correctly() {
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

    // Store original environment for comparison
    let original_env: HashMap<String, String> = std::env::vars().collect();

    // Verify test variables are not initially set
    assert!(
        std::env::var("TEST_VAR").is_err(),
        "TEST_VAR should not be set initially"
    );
    assert!(
        std::env::var("CUENV_TEST").is_err(),
        "CUENV_TEST should not be set initially"
    );

    // Load environment from the test directory
    env_manager.load_env(&test_dir).await.unwrap();

    // Verify variables are loaded (note: these may be stored in state, not directly in env)
    let state_manager = StateManager::new();
    let state = state_manager.get_current_state().await.unwrap();

    if let Some(state) = state {
        assert!(
            state.contains("TEST_VAR"),
            "TEST_VAR should be in loaded state"
        );
        assert!(
            state.contains("CUENV_TEST"),
            "CUENV_TEST should be in loaded state"
        );
    }

    // Unload environment
    env_manager.unload_env().unwrap();

    // Verify environment is restored (no cuenv state should remain)
    let state_after_unload = state_manager.get_current_state().await.unwrap();
    assert!(
        state_after_unload.is_none(),
        "State should be cleared after unload"
    );

    // Verify original environment is preserved
    let final_env: HashMap<String, String> = std::env::vars().collect();

    // Check that TEST_VAR and CUENV_TEST are not present in final environment
    assert!(
        !final_env.contains_key("TEST_VAR"),
        "TEST_VAR should be removed after unload"
    );
    assert!(
        !final_env.contains_key("CUENV_TEST"),
        "CUENV_TEST should be removed after unload"
    );

    println!("✓ Environment loaded and unloaded correctly");
}

#[tokio::test]
async fn test_directory_switching_behavior() {
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
    let state_manager = StateManager::new();

    // Load parent environment
    env_manager.load_env(&parent_dir).await.unwrap();
    let parent_state = state_manager.get_current_state().await.unwrap();
    assert!(
        parent_state.is_some(),
        "Parent environment should be loaded"
    );

    // Switch to child directory
    env_manager.unload_env().unwrap();
    env_manager.load_env(&child_dir).await.unwrap();
    let child_state = state_manager.get_current_state().await.unwrap();
    assert!(child_state.is_some(), "Child environment should be loaded");

    // Leave child directory
    env_manager.unload_env().unwrap();
    let final_state = state_manager.get_current_state().await.unwrap();
    assert!(
        final_state.is_none(),
        "State should be cleared when leaving directory"
    );

    println!("✓ Directory switching behavior works correctly");
}

#[tokio::test]
async fn test_environment_isolation() {
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

    // Load project1, then project2, verify isolation
    env_manager.load_env(&dir1).await.unwrap();
    env_manager.unload_env().unwrap();

    env_manager.load_env(&dir2).await.unwrap();
    env_manager.unload_env().unwrap();

    // Verify no state leakage
    let state_manager = StateManager::new();
    let final_state = state_manager.get_current_state().await.unwrap();
    assert!(
        final_state.is_none(),
        "No state should remain after switching projects"
    );

    println!("✓ Environment isolation works correctly");
}
