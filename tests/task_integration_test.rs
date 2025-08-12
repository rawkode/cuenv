use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn get_cuenv_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("cuenv");
    path
}

// Simple timeout wrapper for commands to avoid hanging integration tests.
fn run_command_with_timeout(
    mut cmd: Command,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    // Capture output
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
                // Process finished, collect remaining output
                return child.wait_with_output();
            }
            None => {
                if start.elapsed() >= timeout {
                    // Timed out: kill and collect output
                    let _ = child.kill();
                    return child.wait_with_output();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn test_task_listing() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package cuenv

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

    let output = run_command_with_timeout(
        {
            let mut cmd = Command::new(get_cuenv_binary());
            cmd.current_dir(temp_dir.path())
                .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
                .arg("run");
            cmd
        },
        std::time::Duration::from_secs(30),
    )
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
        r#"package cuenv

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

    let output = run_command_with_timeout(
        {
            let mut cmd = Command::new(get_cuenv_binary());
            cmd.current_dir(temp_dir.path())
                .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
                .arg("run")
                .arg("hello");
            cmd
        },
        std::time::Duration::from_secs(30),
    )
    .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        eprintln!("Exit code: {}", output.status.code().unwrap_or(-1));
        eprintln!("STDOUT: {stdout}");
        eprintln!("STDERR: {stderr}");
    }
    assert!(output.status.success());
    assert!(stdout.contains("Hello from task"));
}

#[test]
fn test_task_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package cuenv

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

    let output = run_command_with_timeout(
        {
            let mut cmd = Command::new(get_cuenv_binary());
            cmd.current_dir(temp_dir.path())
                .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
                .arg("run")
                .arg("second");
            cmd
        },
        std::time::Duration::from_secs(30),
    )
    .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("First task"));
    assert!(stdout.contains("Second task"));

    // Ensure proper order: "First task" should appear before "Second task"
    let first_pos = stdout.find("First task").unwrap();
    let second_pos = stdout.find("Second task").unwrap();
    assert!(first_pos < second_pos);
}

#[test]
fn test_missing_task_error() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package cuenv

env: {}

tasks: {
    "existing": {
        command: "echo 'exists'"
    }
}"#,
    )
    .unwrap();

    let output = run_command_with_timeout(
        {
            let mut cmd = Command::new(get_cuenv_binary());
            cmd.current_dir(temp_dir.path())
                .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
                .arg("run")
                .arg("nonexistent");
            cmd
        },
        std::time::Duration::from_secs(30),
    )
    .expect("Failed to run cuenv");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // When a task is not found, cuenv tries to run it as a command
    // which results in "Failed to spawn command" error
    assert!(
        stderr.contains("Failed to spawn command") || stderr.contains("not found"),
        "Expected error in stderr, got: '{stderr}'"
    );
}

#[test]
fn test_task_with_script() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package cuenv

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

    let output = run_command_with_timeout(
        {
            let mut cmd = Command::new(get_cuenv_binary());
            cmd.current_dir(temp_dir.path())
                .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
                .arg("run")
                .arg("script-task");
            cmd
        },
        std::time::Duration::from_secs(30),
    )
    .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Script start"));
    assert!(stdout.contains("TEST_VAR is: script-test"));
    assert!(stdout.contains("Script end"));
}
