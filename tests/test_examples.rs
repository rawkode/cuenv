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

fn get_example_path(example: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("examples");
    path.push(example);
    path
}

#[test]
fn test_basic_env_example() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env.cue");

    // Test basic load
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .env("CUENV_FILE", "env.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(
        output.status.success(),
        "cuenv load failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that basic variables are loaded (in export format)
    assert!(stdout.contains("export DATABASE_NAME="));
    assert!(stdout.contains("export PORT="));
    assert!(stdout.contains("export LOG_LEVEL="));

    // Check that secrets are present (they'll be unresolved in load)
    assert!(stdout.contains("export AWS_ACCESS_KEY="));
    assert!(stdout.contains("export AWS_SECRET_KEY="));
}

#[test]
fn test_env_with_capabilities() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env-with-capabilities.cue");

    // Test load without capabilities (includes all vars in current implementation)
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .env("CUENV_FILE", "env-with-capabilities.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have all vars (capability filtering only works with explicit -c flag)
    assert!(stdout.contains("export DATABASE_NAME="));
    assert!(stdout.contains("export AWS_ACCESS_KEY="));

    // Test load with aws capability
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .arg("-c")
        .arg("aws")
        .env("CUENV_FILE", "env-with-capabilities.cue")
        .output()
        .expect("Failed to execute cuenv load with capability");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should now include AWS vars
    assert!(stdout.contains("export AWS_ACCESS_KEY="));
    assert!(stdout.contains("export AWS_SECRET_KEY="));
}

#[test]
fn test_structured_secrets_example() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env-structured-secrets.cue");

    // Test basic load
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .env("CUENV_FILE", "env-structured-secrets.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that variables are present
    assert!(stdout.contains("export DATABASE_NAME="));
    assert!(stdout.contains("export AWS_ACCESS_KEY="));
    assert!(stdout.contains("export DATABASE_PASSWORD="));

    // Check that resolver references are present
    assert!(stdout.contains("cuenv-resolver://"));
}

#[test]
fn test_registry_secrets_example() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env-registry-secrets.cue");

    // Test basic load
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .env("CUENV_FILE", "env-registry-secrets.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check various secret types
    assert!(stdout.contains("export DATABASE_PASSWORD="));
    assert!(stdout.contains("export AWS_ACCESS_KEY="));
    assert!(stdout.contains("export API_KEY="));
    assert!(stdout.contains("export ENCRYPTION_KEY="));
}

#[test]
fn test_environment_overrides() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env-structured-secrets.cue");

    // Test with production environment
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .arg("-e")
        .arg("production")
        .env("CUENV_FILE", "env-structured-secrets.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have production-specific values
    assert!(stdout.contains("export DATABASE_URL="));
    assert!(stdout.contains("production:5432"));
}

#[test]
fn test_secret_resolution_with_echo() {
    let cuenv = get_cuenv_binary();
    let temp_dir = TempDir::new().unwrap();

    // Create a test CUE file with echo-based secrets
    let test_cue = r#"package env

#SecretRef: {
    resolver: {
        command: string
        args: [...string]
    }
    ...
}

#EchoSecret: #SecretRef & {
    value: string
    resolver: {
        command: "echo"
        args: [value]
    }
}

NORMAL_VAR: "plain-value"
TEST_SECRET: #EchoSecret & { value: "secret-123" }
"#;

    let cue_path = temp_dir.path().join("env.cue");
    std::fs::write(&cue_path, test_cue).unwrap();

    // Test that secrets are resolved during run command
    let output = Command::new(&cuenv)
        .arg("run")
        .arg("env")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute cuenv run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that the secret was resolved (it will be masked in output)
    assert!(
        stdout.contains("TEST_SECRET=***********"),
        "Expected masked secret in output"
    );
    assert!(stdout.contains("NORMAL_VAR=plain-value"));
}

#[test]
fn test_multiple_capabilities() {
    let cuenv = get_cuenv_binary();
    let example = get_example_path("env-structured-secrets.cue");

    // Test with multiple capabilities
    let output = Command::new(&cuenv)
        .arg("load")
        .arg("-d")
        .arg(example.parent().unwrap())
        .arg("-c")
        .arg("aws")
        .arg("-c")
        .arg("stripe")
        .env("CUENV_FILE", "env-structured-secrets.cue")
        .output()
        .expect("Failed to execute cuenv load");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should include both AWS and Stripe vars
    assert!(stdout.contains("export AWS_ACCESS_KEY="));
    assert!(stdout.contains("export STRIPE_KEY="));
}

#[test]
fn test_invalid_cue_syntax() {
    let cuenv = get_cuenv_binary();
    let temp_dir = TempDir::new().unwrap();

    // Create invalid CUE file
    let invalid_cue = r#"package env
INVALID_SYNTAX: {
    missing: "closing brace"
"#;

    let cue_path = temp_dir.path().join("env.cue");
    std::fs::write(&cue_path, invalid_cue).unwrap();

    let output = Command::new(&cuenv)
        .arg("load")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute cuenv load");

    // Should fail with error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("Error"));
}
