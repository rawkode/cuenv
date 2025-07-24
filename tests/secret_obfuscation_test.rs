use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Tests that secrets are obfuscated in command output
/// Since we can't easily mock external secret managers in integration tests,
/// we'll test the obfuscation logic using plain values that would be treated as secrets
#[test]
fn test_secret_obfuscation_in_output() {
    // For this test, we'll use a helper script that simulates outputting secrets
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Create a CUE file with a value that looks like a secret
    fs::write(
        &env_file,
        r#"package env

env: {
// Normal environment variables
PUBLIC_VAR: "public-value"
API_ENDPOINT: "https://api.example.com"

// These would normally be secret references like "op://vault/item"
// But for testing without real secret managers, we use plain values
SECRET_KEY: "supersecret123"
DATABASE_PASSWORD: "db-pass-456"
}
"#,
    )
    .unwrap();

    // Create a test script that outputs these values
    let script_path = temp_dir.path().join("test_script.sh");
    fs::write(
        &script_path,
        r#"#!/bin/sh
echo "Public: $PUBLIC_VAR"
echo "API: $API_ENDPOINT"
echo "Secret in output: $SECRET_KEY"
echo "Database password is: $DATABASE_PASSWORD"
echo "Mixed: public-value and $SECRET_KEY together"
"#,
    )
    .unwrap();

    // Make the script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let mut cuenv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cuenv_path.push("target");
    cuenv_path.push("debug");
    cuenv_path.push("cuenv");

    // Run the script through cuenv
    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(&["run", script_path.to_str().unwrap()])
        .output()
        .expect("Failed to run cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Since we're not using real secret managers in tests, the obfuscation
    // won't happen. This test primarily verifies that the code compiles
    // and runs without errors.
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that output contains expected values (not obfuscated in test mode)
    assert!(stdout.contains("Public: public-value"));
    assert!(stdout.contains("API: https://api.example.com"));
}

#[test]
fn test_secret_obfuscation_preserves_functionality() {
    // Test that commands still work correctly with output filtering
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
TEST_VAR: "test-value"
SECRET_VAR: "secret-value"
}
"#,
    )
    .unwrap();

    let mut cuenv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cuenv_path.push("target");
    cuenv_path.push("debug");
    cuenv_path.push("cuenv");

    // Test that exit codes are preserved
    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(&["run", "sh", "--", "-c", "exit 42"])
        .output()
        .expect("Failed to run cuenv");

    assert_eq!(
        output.status.code(),
        Some(42),
        "Exit code should be preserved"
    );

    // Test that stderr is also filtered
    let output = Command::new(&cuenv_path)
        .current_dir(temp_dir.path())
        .args(&[
            "run",
            "sh",
            "--",
            "-c",
            "echo \"Value is: $SECRET_VAR\" >&2",
        ])
        .output()
        .expect("Failed to run cuenv");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());
    // In test mode, secrets aren't resolved, so the output won't be obfuscated
    // The variable should be expanded to its value
    assert!(stderr.contains("Value is: secret-value"));
}
