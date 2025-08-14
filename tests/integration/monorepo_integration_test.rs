use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to run cuenv command in a directory
fn run_cuenv(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    // Get the project root directory
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir);

    // Build the path to the cuenv binary
    let exe_path = project_root.join("target").join("debug").join("cuenv");

    let mut cmd = Command::new(exe_path);
    cmd.args(args).current_dir(dir).env("RUST_BACKTRACE", "1");

    cmd.output().expect("Failed to execute cuenv")
}

/// Test discovering packages in a monorepo
#[test]
fn test_monorepo_discover_command() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Root package
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }
tasks: {
    "deploy": {
        command: "echo 'deploying'"
        description: "Deploy all services"
    }
}"#,
    )
    .unwrap();

    // Frontend package
    fs::create_dir_all(root.join("frontend")).unwrap();
    fs::write(
        root.join("frontend/env.cue"),
        r#"package cuenv
env: { FRONTEND: "true" }
tasks: {
    "build": {
        command: "echo 'building frontend'"
        description: "Build frontend"
    }
}"#,
    )
    .unwrap();

    // Backend package
    fs::create_dir_all(root.join("backend")).unwrap();
    fs::write(
        root.join("backend/env.cue"),
        r#"package cuenv
env: { BACKEND: "true" }
tasks: {
    "build": {
        command: "echo 'building backend'"
        description: "Build backend"
    }
}"#,
    )
    .unwrap();

    // Run discover command with --dump to see tasks
    let output = run_cuenv(root, &["discover", "--dump"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Debug output
    println!("stdout: {stdout}");
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Check output contains all packages
    assert!(stdout.contains("root"), "Should list root package");
    assert!(stdout.contains("frontend"), "Should list frontend package");
    assert!(stdout.contains("backend"), "Should list backend package");

    // Check tasks are listed (within their package sections)
    assert!(stdout.contains("deploy"), "Should list deploy task");
    assert!(stdout.contains("build"), "Should list build tasks");

    // Check that frontend package has build task
    let frontend_section = stdout.find("Package: frontend").unwrap();
    let frontend_build = stdout[frontend_section..].find("build").unwrap();
    assert!(frontend_build > 0, "Frontend should have build task");

    // Check that backend package has build task
    let backend_section = stdout.find("Package: backend").unwrap();
    let backend_build = stdout[backend_section..].find("build").unwrap();
    assert!(backend_build > 0, "Backend should have build task");
}

/// Test running cross-package tasks
#[test]
fn test_monorepo_task_execution() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Root package (required for discovery to work)
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }"#,
    )
    .unwrap();

    // Library package
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib/env.cue"),
        r#"package cuenv
env: { LIB: "true" }
tasks: {
    "build": {
        command: "mkdir -p dist && echo 'library built' > dist/lib.txt"
        description: "Build library"
        outputs: ["dist/lib.txt"]
    }
}"#,
    )
    .unwrap();

    // App package that depends on lib
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/env.cue"),
        r#"package cuenv
env: { APP: "true" }
tasks: {
    "build": {
        command: "echo 'app built' > app.txt"
        description: "Build app"
        dependencies: ["lib:build"]
    }
}"#,
    )
    .unwrap();

    // Run task with cross-package dependency
    let output = run_cuenv(root, &["run", "app:build"]);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify both tasks executed
    assert!(
        root.join("lib/dist/lib.txt").exists(),
        "Library should be built"
    );
    assert!(root.join("app/app.txt").exists(), "App should be built");

    // Verify content
    let lib_content = fs::read_to_string(root.join("lib/dist/lib.txt")).unwrap();
    assert!(lib_content.contains("library built"));

    let app_content = fs::read_to_string(root.join("app/app.txt")).unwrap();
    assert!(app_content.contains("app built"));
}

/// Test listing tasks across packages
#[test]
fn test_monorepo_task_list() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create multiple packages with tasks
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }
tasks: {
    "clean": {
        command: "echo 'cleaning'"
        description: "Clean all"
    }
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("services/api")).unwrap();
    fs::write(
        root.join("services/api/env.cue"),
        r#"package cuenv
env: { API: "true" }
tasks: {
    "test": {
        command: "echo 'testing api'"
        description: "Test API"
    }
    "build": {
        command: "echo 'building api'"
        description: "Build API"
    }
}"#,
    )
    .unwrap();

    // Run command without arguments to list tasks
    let output = run_cuenv(root, &["run"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check all tasks are listed with proper names
    assert!(stdout.contains("root:clean"), "Should list root:clean");
    assert!(
        stdout.contains("services:api:test"),
        "Should list services:api:test"
    );
    assert!(
        stdout.contains("services:api:build"),
        "Should list services:api:build"
    );

    // Check descriptions are shown
    assert!(stdout.contains("Clean all"));
    assert!(stdout.contains("Test API"));
    assert!(stdout.contains("Build API"));
}

/// Test running task from subdirectory
#[test]
fn test_run_task_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Root package (required for discovery to work)
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }"#,
    )
    .unwrap();

    // Create packages
    fs::create_dir_all(root.join("packages/web")).unwrap();
    fs::write(
        root.join("packages/web/env.cue"),
        r#"package cuenv
env: { WEB: "true" }
tasks: {
    "build": {
        command: "echo 'web built' > build.txt"
        description: "Build web"
    }
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("packages/api")).unwrap();
    fs::write(
        root.join("packages/api/env.cue"),
        r#"package cuenv
env: { API: "true" }
tasks: {
    "build": {
        command: "echo 'api built' > build.txt"
        description: "Build API"
        dependencies: ["packages:web:build"]
    }
}"#,
    )
    .unwrap();

    // Run task from api subdirectory
    let api_dir = root.join("packages/api");
    let output = run_cuenv(&api_dir, &["run", "build"]);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify both tasks executed
    assert!(
        root.join("packages/web/build.txt").exists(),
        "Web should be built"
    );
    assert!(
        root.join("packages/api/build.txt").exists(),
        "API should be built"
    );
}

/// Test circular dependency detection
#[test]
fn test_circular_dependency_detection() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo with circular dependencies
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Root package (required for discovery to work)
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(
        root.join("a/env.cue"),
        r#"package cuenv
env: { A: "true" }
tasks: {
    "build": {
        command: "echo 'a'"
        dependencies: ["b:build"]
    }
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("b")).unwrap();
    fs::write(
        root.join("b/env.cue"),
        r#"package cuenv
env: { B: "true" }
tasks: {
    "build": {
        command: "echo 'b'"
        dependencies: ["a:build"]
    }
}"#,
    )
    .unwrap();

    // Try to run task with circular dependency
    let output = run_cuenv(root, &["run", "a:build"]);

    // Should fail
    assert!(
        !output.status.success(),
        "Should fail with circular dependency"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Circular") || stderr.contains("circular"),
        "Should mention circular dependency: {stderr}"
    );
}

/// Test task with cross-package output dependencies
#[test]
fn test_task_with_cross_package_outputs() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Root package (required for discovery to work)
    fs::write(
        root.join("env.cue"),
        r#"package cuenv
env: { ROOT: "true" }"#,
    )
    .unwrap();

    // Data generator package
    fs::create_dir_all(root.join("generator")).unwrap();
    fs::write(
        root.join("generator/env.cue"),
        r#"package cuenv
env: { GENERATOR: "true" }
tasks: {
    "generate": {
        command: "mkdir -p data && echo 'test data' > data/input.txt"
        description: "Generate data"
        outputs: ["data/input.txt"]
    }
}"#,
    )
    .unwrap();

    // Processor package that uses generated data
    fs::create_dir_all(root.join("processor")).unwrap();

    // Create a simple processing script that uses relative paths
    let script_content = if cfg!(target_os = "windows") {
        r#"@echo off
if exist "..\generator\data\input.txt" (
    type "..\generator\data\input.txt" > processed.txt
    echo Processing complete >> processed.txt
) else (
    echo No input data > processed.txt
)
"#
    } else {
        r#"#!/bin/sh
if [ -f "../generator/data/input.txt" ]; then
    cat "../generator/data/input.txt" > processed.txt
    echo "Processing complete" >> processed.txt
else
    echo "No input data" > processed.txt
fi
"#
    };

    let script_name = if cfg!(target_os = "windows") {
        "process.bat"
    } else {
        "process.sh"
    };

    fs::write(root.join("processor").join(script_name), script_content).unwrap();

    // Make script executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script_path = root.join("processor").join(script_name);
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    fs::write(
        root.join("processor/env.cue"),
        format!(
            r#"package cuenv
env: {{ PROCESSOR: "true" }}
tasks: {{
    "process": {{
        script: "{script_name}"
        description: "Process data"
        dependencies: ["generator:generate"]
    }}
}}"#
        ),
    )
    .unwrap();

    // Run the processing task
    let output = run_cuenv(root, &["run", "processor:process"]);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the processed file was created
    let processed_file = root.join("processor/processed.txt");
    assert!(processed_file.exists(), "Processed file should exist");

    let content = fs::read_to_string(processed_file).unwrap();
    assert!(
        content.contains("test data"),
        "Should contain original data"
    );
    assert!(
        content.contains("Processing complete"),
        "Should be processed"
    );
}
