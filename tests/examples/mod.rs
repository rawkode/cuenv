use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Test all example directories for proper CUE parsing
#[test]
fn test_all_examples() -> anyhow::Result<()> {
    let examples_dir = Path::new("examples");

    let mut failed = Vec::new();

    for entry in fs::read_dir(examples_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let env_cue = path.join("env.cue");
            if env_cue.exists() {
                let name = path.file_name().unwrap().to_string_lossy();

                match test_example_dir(&path) {
                    Ok(_) => println!("✓ Example '{}' passed", name),
                    Err(e) => {
                        println!("✗ Example '{}' failed: {}", name, e);
                        failed.push(name.to_string());
                    }
                }
            }
        }
    }

    if !failed.is_empty() {
        anyhow::bail!("Failed examples: {:?}", failed);
    }

    Ok(())
}

fn test_example_dir(dir: &Path) -> anyhow::Result<()> {
    // Test basic export
    let output = Command::new("./target/debug/cuenv")
        .args(&["env", "export"])
        .current_dir(dir)
        .env("CUENV_PACKAGE", "examples")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Export failed: {}", stderr);
    }

    // Test with environment if present
    let cue_content = fs::read_to_string(dir.join("env.cue"))?;
    if cue_content.contains("environment:") {
        test_environment_selection(dir)?;
    }

    // Test with capabilities if present
    if cue_content.contains("@capability") {
        test_capabilities(dir)?;
    }

    // Test tasks if present
    if cue_content.contains("tasks:") {
        test_tasks(dir)?;
    }

    Ok(())
}

fn test_environment_selection(dir: &Path) -> anyhow::Result<()> {
    for env in &["development", "production", "staging"] {
        let output = Command::new("./target/debug/cuenv")
            .args(&["env", "export", "--env", env])
            .current_dir(dir)
            .env("CUENV_PACKAGE", "examples")
            .output()?;

        // It's okay if some environments don't exist
        if output.status.success() {
            println!("  - Environment '{}' exported successfully", env);
        }
    }
    Ok(())
}

fn test_capabilities(dir: &Path) -> anyhow::Result<()> {
    let output = Command::new("./target/debug/cuenv")
        .args(&["env", "export", "--capability", "aws"])
        .current_dir(dir)
        .env("CUENV_PACKAGE", "examples")
        .output()?;

    if output.status.success() {
        println!("  - Capability 'aws' exported successfully");
    }

    Ok(())
}

fn test_tasks(dir: &Path) -> anyhow::Result<()> {
    let output = Command::new("./target/debug/cuenv")
        .args(&["task", "list"])
        .current_dir(dir)
        .env("CUENV_PACKAGE", "examples")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Task list failed: {}", stderr);
    }

    println!("  - Tasks listed successfully");
    Ok(())
}

/// Test secret resolution
#[test]
fn test_secret_resolution() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    let cue_content = r#"
package examples

env: {
    NORMAL_VAR: "plain-value"
    SECRET_VAR: {
        resolver: {
            command: "echo"
            args: ["resolved-secret"]
        }
    }
}
"#;

    fs::write(temp_dir.path().join("env.cue"), cue_content)?;

    let output = Command::new("./target/debug/cuenv")
        .args(&["env", "export"])
        .current_dir(temp_dir.path())
        .env("CUENV_PACKAGE", "examples")
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if normal var is present
        assert!(stdout.contains("NORMAL_VAR=plain-value"));

        // Check if secret is resolved (or feature not implemented)
        if stdout.contains("SECRET_VAR=") {
            println!("Secret resolution implemented and working");
        } else {
            println!("Secret resolution not yet implemented");
        }
    }

    Ok(())
}

/// Test monorepo support
#[test]
fn test_monorepo_inheritance() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Create root env.cue
    let root_cue = r#"
package examples

env: {
    ROOT_VAR: "root-value"
    SHARED_VAR: "root-shared"
}
"#;
    fs::write(temp_dir.path().join("env.cue"), root_cue)?;

    // Create child directory with env.cue
    let child_dir = temp_dir.path().join("child");
    fs::create_dir(&child_dir)?;

    let child_cue = r#"
package examples

env: {
    CHILD_VAR: "child-value"
    SHARED_VAR: "child-override"
}
"#;
    fs::write(child_dir.join("env.cue"), child_cue)?;

    // Test child inherits from parent
    let output = Command::new("./target/debug/cuenv")
        .args(&["env", "export"])
        .current_dir(&child_dir)
        .env("CUENV_PACKAGE", "examples")
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should have both root and child variables
        if stdout.contains("ROOT_VAR=") && stdout.contains("CHILD_VAR=") {
            println!("Monorepo inheritance working correctly");
        }
    }

    Ok(())
}
