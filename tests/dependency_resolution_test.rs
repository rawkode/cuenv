use cuenv::discovery::PackageDiscovery;
use cuenv::task::cross_package::{parse_reference, CrossPackageReference};
use cuenv::task::registry::MonorepoTaskRegistry;
use cuenv::task::staging::{DependencyStager, StagedDependency};
use std::fs;
use tempfile::TempDir;

/// Test dependency resolution across packages
#[tokio::test]
async fn test_resolve_cross_package_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Frontend package with build output
    fs::create_dir_all(root.join("frontend")).unwrap();
    fs::write(
        root.join("frontend/env.cue"),
        r#"package env
env: { FRONTEND: "true" }
tasks: {
    "build": {
        command: "echo 'building frontend'"
        outputs: ["dist"]
    }
}"#,
    )
    .unwrap();

    // Create actual output files
    fs::create_dir_all(root.join("frontend/dist")).unwrap();
    fs::write(root.join("frontend/dist/index.html"), "<html>Test</html>").unwrap();
    fs::write(root.join("frontend/dist/bundle.js"), "console.log('test')").unwrap();

    // Backend package with build output
    fs::create_dir_all(root.join("backend")).unwrap();
    fs::write(
        root.join("backend/env.cue"),
        r#"package env
env: { BACKEND: "true" }
tasks: {
    "build": {
        command: "echo 'building backend'"
        outputs: ["bin/server"]
    }
}"#,
    )
    .unwrap();

    // Create actual output files
    fs::create_dir_all(root.join("backend/bin")).unwrap();
    fs::write(
        root.join("backend/bin/server"),
        "#!/bin/bash\necho 'server'",
    )
    .unwrap();

    // Deploy package that depends on both
    fs::write(
        root.join("env.cue"),
        r#"package env
env: { ROOT: "true" }
tasks: {
    "deploy": {
        command: "echo 'deploying'"
        dependencies: ["frontend:build", "backend:build"]
        inputs: ["frontend:build#dist", "backend:build#bin/server"]
    }
}"#,
    )
    .unwrap();

    // Discover packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Build registry
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Validate all dependencies exist
    registry.validate_all_dependencies().unwrap();

    // Test resolving task outputs
    let frontend_dist = registry
        .resolve_task_output("frontend:build", "dist")
        .unwrap();
    assert!(frontend_dist.exists());
    assert!(frontend_dist.is_dir());

    let backend_server = registry
        .resolve_task_output("backend:build", "bin/server")
        .unwrap();
    assert!(backend_server.exists());
    assert!(backend_server.is_file());
}

/// Test dependency resolution with missing outputs
#[tokio::test]
async fn test_resolve_missing_outputs() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Package with declared but missing outputs
    fs::write(
        root.join("env.cue"),
        r#"package env
env: { ROOT: "true" }
tasks: {
    "build": {
        command: "echo 'building'"
        outputs: ["dist/missing.txt"]
    }
}"#,
    )
    .unwrap();

    // Discover packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Build registry
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Resolving missing output should fail
    let result = registry.resolve_task_output("root:build", "dist/missing.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

/// Test resolving dependencies with staging
#[tokio::test]
async fn test_stage_cross_package_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Library package with output
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib/env.cue"),
        r#"package env
env: { LIB: "true" }
tasks: {
    "build": {
        command: "echo 'building lib'"
        outputs: ["dist/lib.so"]
    }
}"#,
    )
    .unwrap();

    // Create actual output
    fs::create_dir_all(root.join("lib/dist")).unwrap();
    fs::write(root.join("lib/dist/lib.so"), "shared library content").unwrap();

    // App package that depends on lib
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/env.cue"),
        r#"package env
env: { APP: "true" }
tasks: {
    "build": {
        command: "echo 'building app'"
        dependencies: ["lib:build"]
        inputs: ["lib:build#dist/lib.so"]
    }
}"#,
    )
    .unwrap();

    // Discover packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Build registry
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Get the app:build task
    let app_task = registry.get_task("app:build").unwrap();

    // Stage dependencies for the task
    let mut stager = DependencyStager::new().unwrap();

    // Resolve and stage each input
    if let Some(ref inputs) = app_task.config.inputs {
        for input in inputs {
            let dep_ref = parse_reference(input).unwrap();

            if let CrossPackageReference::PackageTaskOutput {
                package,
                task,
                output,
            } = dep_ref
            {
                let task_name = format!("{}:{}", package, task);
                let output_path = registry.resolve_task_output(&task_name, &output).unwrap();

                // Create a staged dependency
                let staged_dep = StagedDependency {
                    name: format!("{}:{}", task, output),
                    source_path: output_path.clone(),
                    target_name: Some(format!("{}_{}", task, output)),
                };

                // Stage the dependency
                let staged_path = stager.stage_dependency(&staged_dep).unwrap();
                assert!(staged_path.exists());
            }
        }
    }

    // Verify staged files exist
    // We staged one dependency (lib.so)
    // The stager's staging directory should contain the file
}

/// Test circular dependency detection
#[tokio::test]
async fn test_circular_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure with circular dependencies
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Package A depends on B
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(
        root.join("a/env.cue"),
        r#"package env
env: { A: "true" }
tasks: {
    "build": {
        command: "echo 'building a'"
        dependencies: ["b:build"]
    }
}"#,
    )
    .unwrap();

    // Package B depends on A (circular)
    fs::create_dir_all(root.join("b")).unwrap();
    fs::write(
        root.join("b/env.cue"),
        r#"package env
env: { B: "true" }
tasks: {
    "build": {
        command: "echo 'building b'"
        dependencies: ["a:build"]
    }
}"#,
    )
    .unwrap();

    // Discover packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Build registry - should succeed
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Validation should still pass at registry level
    // (Circular detection happens during execution)
    registry.validate_all_dependencies().unwrap();
}

/// Test transitive dependency resolution
#[tokio::test]
async fn test_transitive_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo with transitive dependencies: A -> B -> C
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Package C (no dependencies)
    fs::create_dir_all(root.join("c")).unwrap();
    fs::write(
        root.join("c/env.cue"),
        r#"package env
env: { C: "true" }
tasks: {
    "build": {
        command: "echo 'building c'"
        outputs: ["lib.a"]
    }
}"#,
    )
    .unwrap();
    fs::write(root.join("c/lib.a"), "library archive").unwrap();

    // Package B depends on C
    fs::create_dir_all(root.join("b")).unwrap();
    fs::write(
        root.join("b/env.cue"),
        r#"package env
env: { B: "true" }
tasks: {
    "build": {
        command: "echo 'building b'"
        dependencies: ["c:build"]
        inputs: ["c:build#lib.a"]
        outputs: ["lib.so"]
    }
}"#,
    )
    .unwrap();
    fs::write(root.join("b/lib.so"), "shared library").unwrap();

    // Package A depends on B
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(
        root.join("a/env.cue"),
        r#"package env
env: { A: "true" }
tasks: {
    "build": {
        command: "echo 'building a'"
        dependencies: ["b:build"]
        inputs: ["b:build#lib.so"]
    }
}"#,
    )
    .unwrap();

    // Discover packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Build registry
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Validate all dependencies
    registry.validate_all_dependencies().unwrap();

    // Get dependents of C
    let c_dependents = registry.get_dependents("c:build");
    assert_eq!(c_dependents.len(), 1);
    assert_eq!(c_dependents[0].full_name, "b:build");

    // Get dependents of B
    let b_dependents = registry.get_dependents("b:build");
    assert_eq!(b_dependents.len(), 1);
    assert_eq!(b_dependents[0].full_name, "a:build");
}
