/// End-to-end acceptance tests for the preload supervisor
/// These tests simulate real-world usage scenarios
use cuenv_config::Hook;
use cuenv_env::manager::environment::preload_supervisor::{run_supervisor, CapturedEnvironment};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::time::sleep;

/// Simulate a real development environment setup scenario
#[tokio::test]
async fn test_e2e_development_environment_setup() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create a nix-like environment script
    let nix_script = temp_dir.path().join("nix-env.sh");
    fs::write(
        &nix_script,
        r#"#!/bin/bash
# Simulate nix print-dev-env output
echo "export PATH=/nix/store/abc123-nodejs/bin:/nix/store/def456-rust/bin:$PATH"
echo "export NODE_VERSION=20.0.0"
echo "export RUST_VERSION=1.75.0"
echo "export CARGO_HOME=/tmp/.cargo"
echo "export NPM_CONFIG_PREFIX=/tmp/.npm"
"#,
    )
    .unwrap();

    // Create a project initialization script
    let init_script = temp_dir.path().join("init-project.sh");
    fs::write(
        &init_script,
        r#"#!/bin/bash
# Simulate project initialization
mkdir -p node_modules
mkdir -p target
echo "Project initialized" > .init-marker
"#,
    )
    .unwrap();

    // Create a dependency check script
    let deps_script = temp_dir.path().join("check-deps.sh");
    fs::write(
        &deps_script,
        r#"#!/bin/bash
# Simulate dependency checking
echo "Checking dependencies..."
sleep 0.1
echo "Dependencies OK"
"#,
    )
    .unwrap();

    // Make scripts executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in [&nix_script, &init_script, &deps_script] {
            let mut perms = fs::metadata(script).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script, perms).unwrap();
        }
    }

    // Create package.json as an input file
    let package_json = project_dir.join("package.json");
    fs::write(
        &package_json,
        r#"{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0"
  }
}"#,
    )
    .unwrap();

    // Create hooks simulating a real development setup
    let hooks = vec![
        // Nix environment setup (source hook)
        Hook {
            command: nix_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: None,
        },
        // Project initialization
        Hook {
            command: init_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(false),
            inputs: Some(vec![package_json.to_string_lossy().to_string()]),
        },
        // Dependency checking
        Hook {
            command: deps_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(false),
            inputs: Some(vec![package_json.to_string_lossy().to_string()]),
        },
    ];

    // Run the supervisor
    let start = SystemTime::now();
    let result = run_supervisor(hooks.clone()).await;
    let elapsed = start.elapsed().unwrap();

    assert!(
        result.is_ok(),
        "Development environment setup should succeed"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Setup should complete within reasonable time"
    );

    // Verify environment was captured
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    let latest_file = cache_dir.join("latest_env.json");

    if latest_file.exists() {
        let content = fs::read_to_string(&latest_file).unwrap();
        let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

        // Verify environment variables were captured
        assert!(captured.env_vars.contains_key("NODE_VERSION"));
        assert!(captured.env_vars.contains_key("RUST_VERSION"));
        assert!(captured.env_vars.contains_key("CARGO_HOME"));
        assert_eq!(
            captured.env_vars.get("NODE_VERSION"),
            Some(&"20.0.0".to_string())
        );
    }

    // Verify project was initialized
    let init_marker = project_dir.join(".init-marker");
    assert!(init_marker.exists(), "Project should be initialized");

    // Run again - should use cache
    let start2 = SystemTime::now();
    let result2 = run_supervisor(hooks).await;
    let elapsed2 = start2.elapsed().unwrap();

    assert!(result2.is_ok(), "Second run should succeed");
    assert!(
        elapsed2 < Duration::from_millis(100),
        "Second run should be fast (cache hit): {:?}",
        elapsed2
    );
}

/// Test a CI/CD pipeline scenario
#[tokio::test]
async fn test_e2e_cicd_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let repo_dir = temp_dir.path().join("repo");
    fs::create_dir_all(&repo_dir).unwrap();

    // Create CI environment setup script
    let ci_env_script = temp_dir.path().join("ci-env.sh");
    fs::write(
        &ci_env_script,
        r#"#!/bin/bash
echo "export CI=true"
echo "export CI_COMMIT_SHA=$(git rev-parse HEAD 2>/dev/null || echo 'mock-sha')"
echo "export CI_BRANCH=$(git branch --show-current 2>/dev/null || echo 'main')"
echo "export BUILD_NUMBER=$RANDOM"
"#,
    )
    .unwrap();

    // Create build script
    let build_script = temp_dir.path().join("build.sh");
    fs::write(
        &build_script,
        r#"#!/bin/bash
echo "Building project..."
sleep 0.2
mkdir -p dist
echo "Build complete" > dist/build.txt
"#,
    )
    .unwrap();

    // Create test script
    let test_script = temp_dir.path().join("test.sh");
    fs::write(
        &test_script,
        r#"#!/bin/bash
echo "Running tests..."
sleep 0.1
echo "Tests passed"
"#,
    )
    .unwrap();

    // Make scripts executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in [&ci_env_script, &build_script, &test_script] {
            let mut perms = fs::metadata(script).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script, perms).unwrap();
        }
    }

    // Create source files as inputs
    let src_file = repo_dir.join("main.rs");
    fs::write(&src_file, "fn main() { println!(\"Hello, CI!\"); }").unwrap();

    let hooks = vec![
        // CI environment setup
        Hook {
            command: ci_env_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(repo_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: None,
        },
        // Build step
        Hook {
            command: build_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(repo_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(false),
            inputs: Some(vec![src_file.to_string_lossy().to_string()]),
        },
        // Test step
        Hook {
            command: test_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(repo_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(false),
            inputs: Some(vec![src_file.to_string_lossy().to_string()]),
        },
    ];

    // Run the CI pipeline
    let result = run_supervisor(hooks.clone()).await;
    assert!(result.is_ok(), "CI pipeline should complete successfully");

    // Verify build artifacts were created
    let build_artifact = repo_dir.join("dist/build.txt");
    assert!(build_artifact.exists(), "Build artifacts should be created");

    // Verify CI environment was captured
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    let latest_file = cache_dir.join("latest_env.json");

    if latest_file.exists() {
        let content = fs::read_to_string(&latest_file).unwrap();
        let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

        assert_eq!(captured.env_vars.get("CI"), Some(&"true".to_string()));
        assert!(captured.env_vars.contains_key("BUILD_NUMBER"));
    }

    // Modify source and run again - should rebuild
    fs::write(&src_file, "fn main() { println!(\"Hello, Updated CI!\"); }").unwrap();

    // Clean build artifacts to verify rebuild
    let _ = fs::remove_dir_all(repo_dir.join("dist"));

    let result2 = run_supervisor(hooks).await;
    assert!(
        result2.is_ok(),
        "CI pipeline should run again after source change"
    );
    assert!(
        build_artifact.exists(),
        "Build should run again after source change"
    );
}

/// Test error recovery and partial completion scenario
#[tokio::test]
async fn test_e2e_error_recovery() {
    let temp_dir = TempDir::new().unwrap();

    // Create a script that succeeds
    let good_script = temp_dir.path().join("good.sh");
    fs::write(
        &good_script,
        r#"#!/bin/bash
echo "export GOOD_VAR=success"
echo "Good script completed" > good.marker
"#,
    )
    .unwrap();

    // Create a script that fails
    let bad_script = temp_dir.path().join("bad.sh");
    fs::write(
        &bad_script,
        r#"#!/bin/bash
echo "Starting bad script" >&2
exit 1
"#,
    )
    .unwrap();

    // Create another script that succeeds
    let recovery_script = temp_dir.path().join("recovery.sh");
    fs::write(
        &recovery_script,
        r#"#!/bin/bash
echo "export RECOVERY_VAR=recovered"
echo "Recovery completed" > recovery.marker
"#,
    )
    .unwrap();

    // Make scripts executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in [&good_script, &bad_script, &recovery_script] {
            let mut perms = fs::metadata(script).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script, perms).unwrap();
        }
    }

    let hooks = vec![
        Hook {
            command: good_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(temp_dir.path().to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: None,
        },
        Hook {
            command: bad_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(temp_dir.path().to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(false),
            inputs: None,
        },
        Hook {
            command: recovery_script.to_string_lossy().to_string(),
            args: None,
            dir: Some(temp_dir.path().to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: None,
        },
    ];

    // Run with partial failure
    let result = run_supervisor(hooks).await;
    assert!(
        result.is_ok(),
        "Supervisor should complete despite individual hook failure"
    );

    // Verify successful hooks completed
    let good_marker = temp_dir.path().join("good.marker");
    let recovery_marker = temp_dir.path().join("recovery.marker");

    assert!(good_marker.exists(), "Good script should have completed");
    assert!(
        recovery_marker.exists(),
        "Recovery script should have completed"
    );

    // Verify environment from successful source hooks was captured
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    let latest_file = cache_dir.join("latest_env.json");

    if latest_file.exists() {
        let content = fs::read_to_string(&latest_file).unwrap();
        let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

        assert_eq!(
            captured.env_vars.get("GOOD_VAR"),
            Some(&"success".to_string())
        );
        assert_eq!(
            captured.env_vars.get("RECOVERY_VAR"),
            Some(&"recovered".to_string())
        );
    }
}

/// Test complex dependency chain with caching
#[tokio::test]
async fn test_e2e_dependency_chain_with_caching() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("complex-project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create dependency files
    let package_json = project_dir.join("package.json");
    fs::write(&package_json, r#"{"name": "test", "version": "1.0.0"}"#).unwrap();

    let cargo_toml = project_dir.join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "test"
version = "0.1.0"
"#,
    )
    .unwrap();

    let requirements_txt = project_dir.join("requirements.txt");
    fs::write(&requirements_txt, "flask==2.0.0\nrequests==2.28.0\n").unwrap();

    // Create setup scripts for each language ecosystem
    let node_setup = temp_dir.path().join("setup-node.sh");
    fs::write(
        &node_setup,
        r#"#!/bin/bash
echo "Setting up Node.js environment..."
echo "export NODE_ENV=development"
echo "export NPM_CONFIG_LOGLEVEL=warn"
touch .node-setup-done
"#,
    )
    .unwrap();

    let rust_setup = temp_dir.path().join("setup-rust.sh");
    fs::write(
        &rust_setup,
        r#"#!/bin/bash
echo "Setting up Rust environment..."
echo "export RUST_BACKTRACE=1"
echo "export CARGO_INCREMENTAL=1"
touch .rust-setup-done
"#,
    )
    .unwrap();

    let python_setup = temp_dir.path().join("setup-python.sh");
    fs::write(
        &python_setup,
        r#"#!/bin/bash
echo "Setting up Python environment..."
echo "export PYTHONPATH=$PWD"
echo "export PIP_DISABLE_VERSION_CHECK=1"
touch .python-setup-done
"#,
    )
    .unwrap();

    // Make scripts executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in [&node_setup, &rust_setup, &python_setup] {
            let mut perms = fs::metadata(script).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script, perms).unwrap();
        }
    }

    let hooks = vec![
        Hook {
            command: node_setup.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: Some(vec![package_json.to_string_lossy().to_string()]),
        },
        Hook {
            command: rust_setup.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: Some(vec![cargo_toml.to_string_lossy().to_string()]),
        },
        Hook {
            command: python_setup.to_string_lossy().to_string(),
            args: None,
            dir: Some(project_dir.to_string_lossy().to_string()),
            preload: Some(true),
            source: Some(true),
            inputs: Some(vec![requirements_txt.to_string_lossy().to_string()]),
        },
    ];

    // First run - all hooks should execute
    let result1 = run_supervisor(hooks.clone()).await;
    assert!(result1.is_ok());

    // Verify all setup markers exist
    assert!(project_dir.join(".node-setup-done").exists());
    assert!(project_dir.join(".rust-setup-done").exists());
    assert!(project_dir.join(".python-setup-done").exists());

    // Clean up markers
    let _ = fs::remove_file(project_dir.join(".node-setup-done"));
    let _ = fs::remove_file(project_dir.join(".rust-setup-done"));
    let _ = fs::remove_file(project_dir.join(".python-setup-done"));

    // Second run without changes - should use cache
    let start = SystemTime::now();
    let result2 = run_supervisor(hooks.clone()).await;
    let elapsed = start.elapsed().unwrap();

    assert!(result2.is_ok());
    assert!(
        elapsed < Duration::from_millis(50),
        "Should be fast due to caching"
    );

    // Markers should NOT be recreated (cached)
    assert!(!project_dir.join(".node-setup-done").exists());
    assert!(!project_dir.join(".rust-setup-done").exists());
    assert!(!project_dir.join(".python-setup-done").exists());

    // Modify one dependency file
    fs::write(&package_json, r#"{"name": "test", "version": "1.0.1"}"#).unwrap();

    // Third run - only Node setup should re-run
    let result3 = run_supervisor(hooks).await;
    assert!(result3.is_ok());

    // Only Node marker should be recreated
    assert!(project_dir.join(".node-setup-done").exists());
    assert!(!project_dir.join(".rust-setup-done").exists());
    assert!(!project_dir.join(".python-setup-done").exists());
}

/// Test real-world Nix flake development scenario
#[tokio::test]
async fn test_e2e_nix_flake_development() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("nix-project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create a mock flake.nix
    let flake_nix = project_dir.join("flake.nix");
    fs::write(
        &flake_nix,
        r#"{
  description = "Test flake";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs";
}"#,
    )
    .unwrap();

    // Create a mock flake.lock
    let flake_lock = project_dir.join("flake.lock");
    fs::write(&flake_lock, r#"{"version": 7, "root": "root"}"#).unwrap();

    // Create nix print-dev-env simulation
    let nix_env_script = temp_dir.path().join("nix-print-dev-env.sh");
    fs::write(
        &nix_env_script,
        r#"#!/bin/bash
# Simulate nix print-dev-env output
cat << 'EOF'
export PATH="/nix/store/abc-nodejs/bin:/nix/store/def-rust/bin:$PATH"
export NIX_CFLAGS_COMPILE="-I/nix/store/include"
export NIX_LDFLAGS="-L/nix/store/lib"
export PKG_CONFIG_PATH="/nix/store/lib/pkgconfig"
export RUST_SRC_PATH="/nix/store/rust-src"
export buildInputs="/nix/store/nodejs /nix/store/rust"
export nativeBuildInputs="/nix/store/gcc /nix/store/pkg-config"
export shellHook='echo "Welcome to nix-shell"'
EOF
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&nix_env_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&nix_env_script, perms).unwrap();
    }

    let hooks = vec![Hook {
        command: nix_env_script.to_string_lossy().to_string(),
        args: None,
        dir: Some(project_dir.to_string_lossy().to_string()),
        preload: Some(true),
        source: Some(true),
        inputs: Some(vec![
            flake_nix.to_string_lossy().to_string(),
            flake_lock.to_string_lossy().to_string(),
        ]),
    }];

    // Run supervisor
    let result = run_supervisor(hooks.clone()).await;
    assert!(result.is_ok(), "Nix environment setup should succeed");

    // Verify environment was captured correctly
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));
    let latest_file = cache_dir.join("latest_env.json");

    assert!(latest_file.exists(), "Environment should be captured");

    let content = fs::read_to_string(&latest_file).unwrap();
    let captured: CapturedEnvironment = serde_json::from_str(&content).unwrap();

    // Verify Nix-specific environment variables
    assert!(captured.env_vars.contains_key("NIX_CFLAGS_COMPILE"));
    assert!(captured.env_vars.contains_key("NIX_LDFLAGS"));
    assert!(captured.env_vars.contains_key("PKG_CONFIG_PATH"));
    assert!(captured.env_vars.contains_key("RUST_SRC_PATH"));
    assert!(captured.env_vars.contains_key("buildInputs"));

    // Run again without changes - should use cache
    let start = SystemTime::now();
    let result2 = run_supervisor(hooks.clone()).await;
    let elapsed = start.elapsed().unwrap();

    assert!(result2.is_ok());
    assert!(
        elapsed < Duration::from_millis(50),
        "Should use cache when flake files unchanged"
    );

    // Update flake.lock (simulating nix flake update)
    fs::write(
        &flake_lock,
        r#"{"version": 7, "root": "root", "updated": true}"#,
    )
    .unwrap();

    // Run again - should detect change and re-execute
    let result3 = run_supervisor(hooks).await;
    assert!(result3.is_ok(), "Should re-run after flake.lock change");
}

/// Clean up test artifacts after all tests
#[test]
fn cleanup_all_test_artifacts() {
    let cache_dir = PathBuf::from(format!(
        "/tmp/cuenv-{}/preload-cache",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));

    if cache_dir.exists() {
        let _ = fs::remove_dir_all(&cache_dir);
    }

    let status_file = PathBuf::from(format!(
        "/tmp/cuenv-{}/hooks-status.json",
        std::env::var("USER").unwrap_or_else(|_| "default".to_string())
    ));

    if status_file.exists() {
        let _ = fs::remove_file(&status_file);
    }
}
