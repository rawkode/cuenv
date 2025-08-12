use crate::manager::{AccessRestrictions, EnvManager};
use cuenv_utils::sync::env::SyncEnv;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_run_command_hermetic() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    fs::write(
        &env_file,
        r#"package cuenv

env: {
    TEST_FROM_CUE: "cue_value"
    PORT: "8080"
}"#,
    )
    .unwrap();

    let mut manager = EnvManager::new();
    manager.load_env(temp_dir.path()).await.unwrap();

    // Set a variable AFTER loading env, so it's not in original_env
    SyncEnv::set_var("TEST_PARENT_VAR", "should_not_exist").unwrap();

    // Run a command that checks for our variables
    #[cfg(unix)]
    let (cmd, args) = (
        "sh",
        vec![
            "-c".to_string(),
            "test \"$TEST_FROM_CUE\" = \"cue_value\" && test -z \"$TEST_PARENT_VAR\"".to_string(),
        ],
    );

    #[cfg(windows)]
    let (cmd, args) = ("cmd", vec![
        "/C".to_string(),
        "if \"%TEST_FROM_CUE%\"==\"cue_value\" (if \"%TEST_PARENT_VAR%\"==\"\" exit 0 else exit 1) else exit 1".to_string()
    ]);

    let status = manager.run_command(cmd, &args).unwrap();

    assert_eq!(status, 0, "Command should succeed with correct environment");

    // Clean up
    let _ = SyncEnv::remove_var("TEST_PARENT_VAR");
}

#[tokio::test]
async fn test_run_command_with_secret_refs() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Write a CUE file with normal values only
    // We can't test actual secret resolution without mocking the secret managers
    fs::write(
        &env_file,
        r#"package cuenv

env: {
    NORMAL_VAR: "normal-value"
    ANOTHER_VAR: "another-value"
}"#,
    )
    .unwrap();

    let mut manager = EnvManager::new();
    manager.load_env(temp_dir.path()).await.unwrap();

    // Run a command that checks the variables
    #[cfg(unix)]
    let (cmd, args) = (
        "sh",
        vec![
            "-c".to_string(),
            "test \"$NORMAL_VAR\" = \"normal-value\" && test \"$ANOTHER_VAR\" = \"another-value\""
                .to_string(),
        ],
    );

    #[cfg(windows)]
    let (cmd, args) = ("cmd", vec![
        "/C".to_string(),
        "if \"%NORMAL_VAR%\"==\"normal-value\" (if \"%ANOTHER_VAR%\"==\"another-value\" exit 0 else exit 1) else exit 1".to_string()
    ]);

    let status = manager.run_command(cmd, &args).unwrap();

    assert_eq!(status, 0, "Command should succeed with all variables set");
}

#[tokio::test]
async fn test_run_command_preserves_path_and_home() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    fs::write(
        &env_file,
        r#"package cuenv

env: {
    TEST_VAR: "test"
}"#,
    )
    .unwrap();

    let mut manager = EnvManager::new();
    manager.load_env(temp_dir.path()).await.unwrap();

    // Run a command that checks PATH and HOME are preserved
    #[cfg(unix)]
    let (cmd, args) = (
        "sh",
        vec![
            "-c".to_string(),
            "test -n \"$PATH\" && test -n \"$HOME\"".to_string(),
        ],
    );

    #[cfg(windows)]
    let (cmd, args) = (
        "cmd",
        vec![
            "/C".to_string(),
            "if defined PATH (if defined HOME exit 0 else exit 1) else exit 1".to_string(),
        ],
    );

    let status = manager.run_command(cmd, &args).unwrap();

    assert_eq!(status, 0, "PATH and HOME should be preserved");
}

#[tokio::test]
async fn test_run_command_with_restrictions() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    fs::write(
        &env_file,
        r#"package cuenv

env: {
    TEST_VAR: "test"
}"#,
    )
    .unwrap();

    let mut manager = EnvManager::new();
    manager.load_env(temp_dir.path()).await.unwrap();

    // Test without restrictions (should work)
    let restrictions = AccessRestrictions::default();
    let status =
        manager.run_command_with_restrictions("echo", &["test".to_string()], &restrictions);

    // This should work since no restrictions are applied
    assert!(
        status.is_ok(),
        "Command should succeed without restrictions"
    );

    // Test with restrictions (may fail in test environment, but should not panic)
    let restrictions = AccessRestrictions::new(true, true);
    let result =
        manager.run_command_with_restrictions("echo", &["test".to_string()], &restrictions);

    // The result may be Ok or Err depending on environment capabilities
    // What matters is that it doesn't panic and properly handles restrictions
    match result {
        Ok(_) => {
            // Command succeeded (unlikely in restricted environment)
        }
        Err(e) => {
            // Command failed due to restrictions (expected in most test environments)
            let error_msg = e.to_string();
            // Verify the error is related to restrictions/command execution
            assert!(
                error_msg.contains("CommandExecution")
                    || error_msg.contains("Failed to capture stdout")
                    || error_msg.contains("Failed to spawn command")
                    || error_msg.contains("Network restrictions with Landlock")
                    || error_msg.contains("configuration error"),
                "Error should be related to command execution with restrictions: {error_msg}"
            );
        }
    }
}
