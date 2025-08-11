use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn get_cuenv_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cuenv"))
}

#[test]
fn test_basic_env_loading() {
    let temp_dir = TempDir::new().unwrap();

    // Create a basic env.cue file in package env
    let env_content = r#"package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "test-api-key"
    PORT: "3000"
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("load")
        .env_clear() // Clear all environment variables to ensure clean test
        .env("PATH", std::env::var("PATH").unwrap_or_default()) // Keep PATH for binary lookup
        .env("HOME", std::env::var("HOME").unwrap_or("/tmp".to_string())) // CUE needs HOME
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export DATABASE_URL="));
    assert!(stdout.contains("export API_KEY="));
    assert!(stdout.contains("export PORT="));
}

#[test]
fn test_capabilities_filtering() {
    let temp_dir = TempDir::new().unwrap();

    let env_content = r#"package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "test-api-key"
    AWS_REGION: "us-east-1" @capability("aws")
    AWS_ACCESS_KEY: "aws-key" @capability("aws")
    DOCKER_REGISTRY: "docker.io" @capability("docker")
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    // Test without capabilities - should include non-capability-tagged vars
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("export")
        .env_clear() // Clear all environment variables to ensure clean test
        .env("PATH", std::env::var("PATH").unwrap_or_default()) // Keep PATH for binary lookup
        .env("HOME", std::env::var("HOME").unwrap_or("/tmp".to_string())) // CUE needs HOME
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export DATABASE_URL="));
    assert!(stdout.contains("export API_KEY="));

    // Test with aws capability - use run command for capability filtering
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("--capability")
        .arg("aws")
        .arg("--")
        .arg("env")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AWS_REGION="));
    assert!(stdout.contains("AWS_ACCESS_KEY="));
    assert!(!stdout.contains("DOCKER_REGISTRY="));
}

#[test]
fn test_environment_overrides() {
    let temp_dir = TempDir::new().unwrap();

    let env_content = r#"package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    PORT: "3000"
    
    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
            PORT: "8080"
        }
        staging: {
            DATABASE_URL: "postgres://staging.example.com/mydb"
        }
}
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    // Test with production environment - use run command for environment selection
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("run")
        .arg("--env")
        .arg("production")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg("echo DATABASE_URL=$DATABASE_URL PORT=$PORT")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("postgres://prod.example.com/mydb"));
    assert!(stdout.contains("8080"));
}

#[test]
fn test_secret_references() {
    let temp_dir = TempDir::new().unwrap();

    let env_content = r#"package env

env: {
DATABASE_URL: "postgres://localhost/mydb"
AWS_ACCESS_KEY: "op://Personal/aws/key"
GITHUB_TOKEN: "github://myorg/myrepo/GITHUB_TOKEN"
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("export")
        .env_clear() // Clear all environment variables to ensure clean test
        .env("PATH", std::env::var("PATH").unwrap_or_default()) // Keep PATH for binary lookup
        .env("HOME", std::env::var("HOME").unwrap_or("/tmp".to_string())) // CUE needs HOME
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Secret references should be passed through as-is in export
    assert!(stdout.contains("op://Personal/aws/key"));
    assert!(stdout.contains("github://myorg/myrepo/GITHUB_TOKEN"));
}

#[test]
fn test_command_capability_inference() {
    let temp_dir = TempDir::new().unwrap();

    let env_content = r#"package env

env: {
DATABASE_URL: "postgres://localhost/mydb"
AWS_ACCESS_KEY: "aws-key" @capability("aws")
DOCKER_REGISTRY: "docker.io" @capability("docker")

capabilities: {
    aws: {
        commands: ["deploy"]
    }
    docker: {
        commands: ["deploy"]
    }
}
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    // When running a command with the deploy capability name, should infer aws and docker capabilities
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("exec")
        .arg("echo")
        .arg("deploy")
        .arg("test")
        .output()
        .expect("Failed to execute command");

    // This should work and include AWS and Docker vars
    if !output.status.success() {
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    assert!(output.status.success());
}

#[test]
fn test_invalid_cue_syntax() {
    let temp_dir = TempDir::new().unwrap();

    let invalid_content = r#"package env

env: {
INVALID_SYNTAX: {
    missing: "closing brace"
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), invalid_content).unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("load")
        .env_clear() // Clear all environment variables to ensure clean test
        .env("PATH", std::env::var("PATH").unwrap_or_default()) // Keep PATH for binary lookup
        .env("HOME", std::env::var("HOME").unwrap_or("/tmp".to_string())) // CUE needs HOME
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("Error"));
}

#[test]
fn test_wrong_package_name() {
    let temp_dir = TempDir::new().unwrap();

    let wrong_package = r#"package wrongname

DATABASE_URL: "postgres://localhost/mydb"
"#;
    std::fs::write(temp_dir.path().join("env.cue"), wrong_package).unwrap();

    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("export")
        .env_clear() // Clear all environment variables to ensure clean test
        .env("PATH", std::env::var("PATH").unwrap_or_default()) // Keep PATH for binary lookup
        .env("HOME", std::env::var("HOME").unwrap_or("/tmp".to_string())) // CUE needs HOME
        .output()
        .expect("Failed to execute command");

    // Should fail because package isn't "env"
    assert!(!output.status.success());
}

#[test]
fn test_run_command_hermetic() {
    let temp_dir = TempDir::new().unwrap();

    let env_content = r#"package env

env: {
TEST_FROM_CUE: "cue_value"
PORT: "8080"
}
"#;
    std::fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();

    // Set a variable that should NOT be passed to the child
    std::env::set_var("TEST_PARENT_VAR", "should_not_exist");

    // Run a command that checks for our variables
    #[cfg(unix)]
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("exec")
        .arg("sh")
        .arg("-c")
        .arg("test \"$TEST_FROM_CUE\" = \"cue_value\" && test -z \"$TEST_PARENT_VAR\"")
        .output()
        .expect("Failed to execute command");

    #[cfg(windows)]
    let output = Command::new(get_cuenv_binary())
        .current_dir(temp_dir.path())
        .arg("exec")
        .arg("cmd")
        .arg("/C")
        .arg("if \"%TEST_FROM_CUE%\"==\"cue_value\" (if \"%TEST_PARENT_VAR%\"==\"\" exit 0 else exit 1) else exit 1")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clean up
    std::env::remove_var("TEST_PARENT_VAR");
}
