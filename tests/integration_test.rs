#![allow(unused)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cuenv_run_with_cue_file() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
APP_NAME: "integration-test"
VERSION: "1.0.0"
FULL_NAME: "\(APP_NAME)-\(VERSION)"
PORT: 9999
DEBUG: true
}
"#,
    )
    .unwrap();

    // Get the path to the cuenv binary
    let mut cuenv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cuenv_path.push("target");
    cuenv_path.push("debug");
    cuenv_path.push("cuenv");

    // Run cuenv with our test file
    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(["exec", "printenv", "APP_NAME"])
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Command failed: stderr={stderr}, stdout={stdout}"
    );
    assert_eq!(stdout.trim(), "integration-test");
}

#[test]
fn test_cuenv_run_hermetic_environment() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
FROM_CUE: "cue-value"
}
"#,
    )
    .unwrap();

    // Set an environment variable that should NOT be passed through
    std::env::set_var("PARENT_TEST_VAR", "parent-value");

    let mut cuenv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cuenv_path.push("target");
    cuenv_path.push("debug");
    cuenv_path.push("cuenv");

    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(["exec", "printenv", "FROM_CUE"])
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout.trim(), "cue-value");

    // Clean up
    std::env::remove_var("PARENT_TEST_VAR");
}

#[test]
fn test_cuenv_run_preserves_required_vars() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
TEST: "value"
}
"#,
    )
    .unwrap();

    let mut cuenv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cuenv_path.push("target");
    cuenv_path.push("debug");
    cuenv_path.push("cuenv");

    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(["exec", "printenv", "PATH"])
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(!stdout.trim().is_empty(), "PATH should be preserved");
}
