use crate::manager::EnvManager;
use cuenv_utils::sync::env::SyncEnv;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_and_unload_env() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    fs::write(
        &env_file,
        r#"package env

env: {
    CUENV_TEST_VAR_UNIQUE: "test_value"
}"#,
    )
    .unwrap();

    let original_value = SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap_or_default();

    let mut manager = EnvManager::new();
    manager.load_env(temp_dir.path()).await.unwrap();

    assert_eq!(
        SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap(),
        Some("test_value".to_string())
    );

    manager.unload_env().unwrap();

    match original_value {
        Some(val) => assert_eq!(SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap(), Some(val)),
        None => assert!(SyncEnv::var("CUENV_TEST_VAR_UNIQUE").unwrap().is_none()),
    }
}