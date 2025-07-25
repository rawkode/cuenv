use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn get_cuenv_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("cuenv");
    path
}

#[test]
fn test_task_listing() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
    APP_NAME: "test-app"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
        dependencies: ["build"]
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Available tasks:"));
    assert!(stdout.contains("build: Build the project"));
    assert!(stdout.contains("test: Run tests"));
}

#[test]
fn test_task_execution() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
    APP_NAME: "test-app"
}

tasks: {
    "hello": {
        description: "Say hello"
        command: "echo 'Hello from task'"
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("hello")
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Hello from task"));
}

#[test]
fn test_task_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {}

tasks: {
    "first": {
        command: "echo 'First task'"
    }
    "second": {
        command: "echo 'Second task'"
        dependencies: ["first"]
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("second")
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("First task"));
    // Check second task output as well
    assert!(stdout.contains("Second task"));
}

#[test]
fn test_task_with_runtime_environment() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
    TEST_VAR: "runtime_test"
}

tasks: {
    "host-runtime-task": {
        description: "Test host runtime"
        command: "echo 'Host runtime works' && env | grep TEST_VAR"
        runtime: {
            type: "host"
        }
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("host-runtime-task")
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);
    }
    
    assert!(output.status.success());
    assert!(stdout.contains("Host runtime works"));
    assert!(stdout.contains("TEST_VAR=runtime_test"));
}

#[test]
fn test_missing_task_error() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {}

tasks: {
    "existing": {
        command: "echo 'exists'"
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("nonexistent")
        .output()
        .expect("Failed to run cuenv");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
}

#[test]
fn test_task_with_script() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
    TEST_VAR: "script-test"
}

tasks: {
    "script-task": {
        description: "Test script execution"
        script: """
            echo "Script start"
            echo "TEST_VAR is: $TEST_VAR"
            echo "Script end"
            """
    }
}"#,
    )
    .unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("script-task")
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Script start"));
    assert!(stdout.contains("TEST_VAR is: script-test"));
    assert!(stdout.contains("Script end"));
}
