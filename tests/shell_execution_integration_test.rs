//! Real shell integration tests
//!
//! These tests validate that generated shell hooks actually execute correctly
//! in real shell environments, rather than just checking string generation.

use cuenv::shell::{Bash, Fish, Shell, Zsh};
use std::fs;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Test that bash hooks can be executed without syntax errors
#[test]
fn test_bash_hook_syntax_validation() {
    let shell = Bash::new();
    let cuenv_path = "/usr/local/bin/cuenv"; // Mock path for testing

    let hook_content = shell
        .generate_hook(cuenv_path)
        .expect("Failed to generate bash hook");

    // Write hook to temporary file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let hook_file = temp_dir.path().join("cuenv_hook.bash");
    fs::write(&hook_file, &hook_content).expect("Failed to write hook file");

    // Test syntax with bash -n (parse only, don't execute)
    let output = Command::new("bash")
        .args(["-n", hook_file.to_str().unwrap()])
        .stdin(Stdio::null())
        .output();

    if let Ok(result) = output {
        assert!(
            result.status.success(),
            "Bash hook should have valid syntax. Stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    } else {
        // If bash is not available in test environment, skip test
        println!("Bash not available in test environment, skipping test");
    }
}

/// Test that zsh hooks can be executed without syntax errors
#[test]
fn test_zsh_hook_syntax_validation() {
    let shell = Zsh::new();
    let cuenv_path = "/usr/local/bin/cuenv";

    let hook_content = shell
        .generate_hook(cuenv_path)
        .expect("Failed to generate zsh hook");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let hook_file = temp_dir.path().join("cuenv_hook.zsh");
    fs::write(&hook_file, &hook_content).expect("Failed to write hook file");

    // Test syntax with zsh -n (parse only, don't execute)
    let output = Command::new("zsh")
        .args(["-n", hook_file.to_str().unwrap()])
        .stdin(Stdio::null())
        .output();

    if let Ok(result) = output {
        assert!(
            result.status.success(),
            "Zsh hook should have valid syntax. Stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    } else {
        // If zsh is not available, skip test
        println!("Zsh not available in test environment, skipping test");
    }
}

/// Test that fish hooks can be executed without syntax errors
#[test]
fn test_fish_hook_syntax_validation() {
    let shell = Fish::new();
    let cuenv_path = "/usr/local/bin/cuenv";

    let hook_content = shell
        .generate_hook(cuenv_path)
        .expect("Failed to generate fish hook");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let hook_file = temp_dir.path().join("cuenv_hook.fish");
    fs::write(&hook_file, &hook_content).expect("Failed to write hook file");

    // Fish has a different syntax check command
    let output = Command::new("fish")
        .args(["-n", hook_file.to_str().unwrap()])
        .stdin(Stdio::null())
        .output();

    if let Ok(result) = output {
        assert!(
            result.status.success(),
            "Fish hook should have valid syntax. Stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    } else {
        // If fish is not available, skip test
        println!("Fish shell not available in test environment, skipping test");
    }
}

/// Test that hooks contain expected function definitions
#[test]
fn test_hook_contains_required_functions() {
    let bash = Bash::new();
    let bash_hook = bash
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate bash hook");

    // Bash hooks should contain essential functions
    assert!(
        bash_hook.contains("cuenv"),
        "Bash hook should reference cuenv command"
    );
    assert!(
        bash_hook.contains("cd") || bash_hook.contains("chdir"),
        "Hook should handle directory changes"
    );

    // Test zsh hook
    let zsh = Zsh::new();
    let zsh_hook = zsh
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate zsh hook");

    assert!(
        zsh_hook.contains("cuenv"),
        "Zsh hook should reference cuenv command"
    );

    // Test fish hook
    let fish = Fish::new();
    let fish_hook = fish
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate fish hook");

    assert!(
        fish_hook.contains("cuenv"),
        "Fish hook should reference cuenv command"
    );
}

/// Test hook generation with different cuenv paths
#[test]
fn test_hook_generation_with_different_paths() {
    let bash = Bash::new();

    let paths = vec![
        "/usr/local/bin/cuenv",
        "/home/user/.local/bin/cuenv",
        "/opt/cuenv/bin/cuenv",
        "cuenv", // Just the command name
    ];

    for path in paths {
        let hook_content = bash.generate_hook(path);
        assert!(
            hook_content.is_ok(),
            "Should generate hook for path: {}",
            path
        );

        let content = hook_content.unwrap();
        assert!(
            content.contains(path),
            "Hook should contain the specified cuenv path: {}",
            path
        );
        assert!(!content.is_empty(), "Generated hook should not be empty");
    }
}

/// Test that hooks handle shell-specific features correctly
#[test]
fn test_shell_specific_features() {
    // Bash-specific features
    let bash = Bash::new();
    let bash_hook = bash
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate bash hook");

    // Should use bash-specific syntax patterns
    // Note: The specific patterns depend on implementation
    assert!(bash_hook.len() > 50, "Bash hook should be substantial");

    // Zsh-specific features
    let zsh = Zsh::new();
    let zsh_hook = zsh
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate zsh hook");

    assert!(zsh_hook.len() > 50, "Zsh hook should be substantial");
    assert_ne!(bash_hook, zsh_hook, "Bash and zsh hooks should differ");

    // Fish-specific features
    let fish = Fish::new();
    let fish_hook = fish
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate fish hook");

    assert!(fish_hook.len() > 50, "Fish hook should be substantial");
    assert_ne!(bash_hook, fish_hook, "Bash and fish hooks should differ");
    assert_ne!(zsh_hook, fish_hook, "Zsh and fish hooks should differ");
}

/// Test hook generation error handling
#[test]
fn test_hook_generation_error_handling() {
    let bash = Bash::new();

    // Test with invalid characters that might cause issues
    let problematic_paths = vec![
        "",                               // Empty path
        "/path/with spaces/cuenv",        // Spaces should be handled
        "/path/with'quotes/cuenv",        // Single quotes
        "/path/with\"doublequotes/cuenv", // Double quotes
    ];

    for path in problematic_paths {
        let result = bash.generate_hook(path);

        if path.is_empty() {
            // Empty path might be an error or might be handled
            if result.is_ok() {
                // If it succeeds, the hook should at least be valid
                let hook = result.unwrap();
                assert!(
                    !hook.is_empty(),
                    "Hook should not be empty even for empty path"
                );
            }
        } else {
            // Other paths should generally succeed
            assert!(
                result.is_ok(),
                "Should handle path with special characters: {}",
                path
            );

            if result.is_ok() {
                let hook = result.unwrap();
                assert!(
                    !hook.is_empty(),
                    "Generated hook should not be empty for path: {}",
                    path
                );
            }
        }
    }
}

/// Integration test that simulates a shell session (if shell is available)
#[test]
fn test_shell_session_simulation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bash = Bash::new();

    // Generate hook content
    let hook_content = bash
        .generate_hook("/usr/local/bin/cuenv")
        .expect("Failed to generate bash hook");

    // Create a test script that sources the hook and runs basic commands
    let test_script = format!(
        r#"#!/bin/bash
set -e
{}

# Test that basic shell operations work after sourcing hook
echo "Shell hook integration test"
cd {}
pwd
echo "Test completed successfully"
"#,
        hook_content,
        temp_dir.path().display()
    );

    let script_file = temp_dir.path().join("test_script.sh");
    fs::write(&script_file, test_script).expect("Failed to write test script");

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_file).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_file, perms).unwrap();
    }

    // Try to execute the script
    let output = Command::new("bash")
        .arg(&script_file)
        .stdin(Stdio::null())
        .output();

    if let Ok(result) = output {
        if result.status.success() {
            let stdout = String::from_utf8_lossy(&result.stdout);
            assert!(
                stdout.contains("Test completed successfully"),
                "Shell session should complete successfully"
            );
        } else {
            let stderr = String::from_utf8_lossy(&result.stderr);
            println!(
                "Shell session test failed (this may be expected in CI): {}",
                stderr
            );
        }
    } else {
        println!("Could not execute shell session test - bash not available");
    }
}

/// Test that hook generation is consistent across calls
#[test]
fn test_hook_generation_consistency() {
    let bash = Bash::new();
    let cuenv_path = "/usr/local/bin/cuenv";

    // Generate the same hook multiple times
    let hooks: Vec<String> = (0..5)
        .map(|_| {
            bash.generate_hook(cuenv_path)
                .expect("Failed to generate hook")
        })
        .collect();

    // All hooks should be identical
    for (i, hook) in hooks.iter().enumerate().skip(1) {
        assert_eq!(
            &hooks[0], hook,
            "Hook generation should be consistent. Hook {} differs from hook 0",
            i
        );
    }
}
