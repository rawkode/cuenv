use cuenv::discovery::PackageDiscovery;
use cuenv::task::{MonorepoTaskRegistry, TaskExecutor};
use std::fs;
use tempfile::TempDir;

/// Test executing a simple task without dependencies
#[tokio::test]
async fn test_execute_simple_task() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create a package with a simple task
    fs::write(
        root.join("env.cue"),
        r#"package env
env: { ROOT: "true" }
tasks: {
    "hello": {
        command: "echo 'Hello World' > output.txt"
    }
}"#,
    )
    .unwrap();

    // Discover and build registry
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Execute the task
    let mut executor = TaskExecutor::new_with_registry(registry).await.unwrap();
    executor.execute("root:hello").await.unwrap();

    // Verify the output file was created
    let output_file = root.join("output.txt");
    assert!(output_file.exists());

    let content = fs::read_to_string(output_file).unwrap();
    assert!(content.contains("Hello World"));
}

/// Test executing tasks with dependencies
#[tokio::test]
async fn test_execute_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create a package with dependent tasks
    fs::write(
        root.join("env.cue"),
        r#"package env
env: { ROOT: "true" }
tasks: {
    "prepare": {
        command: "echo 'Preparing' > prepare.txt"
    }
    "build": {
        command: "echo 'Building' > build.txt"
        dependencies: ["prepare"]
    }
    "test": {
        command: "echo 'Testing' > test.txt"
        dependencies: ["build"]
    }
}"#,
    )
    .unwrap();

    // Discover and build registry
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Execute the test task (should execute prepare -> build -> test)
    let mut executor = TaskExecutor::new_with_registry(registry).await.unwrap();
    executor.execute("root:test").await.unwrap();

    // Verify all files were created in the correct order
    assert!(root.join("prepare.txt").exists());
    assert!(root.join("build.txt").exists());
    assert!(root.join("test.txt").exists());

    // Verify tasks were executed
    assert!(executor.is_executed("root:prepare"));
    assert!(executor.is_executed("root:build"));
    assert!(executor.is_executed("root:test"));
}

/// Test cross-package task execution
#[tokio::test]
async fn test_cross_package_execution() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create lib package
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib/env.cue"),
        r#"package env
env: { LIB: "true" }
tasks: {
    "build": {
        command: "mkdir -p dist && echo 'lib content' > dist/lib.so"
        outputs: ["dist/lib.so"]
    }
}"#,
    )
    .unwrap();

    // Create app package that depends on lib
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/env.cue"),
        r#"package env
env: { APP: "true" }
tasks: {
    "build": {
        command: "echo 'app built' > app.txt"
        dependencies: ["lib:build"]
    }
}"#,
    )
    .unwrap();

    // Discover and build registry
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Execute app:build (should execute lib:build first)
    let mut executor = TaskExecutor::new_with_registry(registry).await.unwrap();
    executor.execute("app:build").await.unwrap();

    // Verify both tasks executed
    assert!(executor.is_executed("lib:build"));
    assert!(executor.is_executed("app:build"));

    // Verify outputs exist
    assert!(root.join("lib/dist/lib.so").exists());
    assert!(root.join("app/app.txt").exists());
}

/// Test execution order calculation
#[tokio::test]
async fn test_execution_order() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo with complex dependencies
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create packages A, B, C with dependencies: A -> B -> C
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(
        root.join("a/env.cue"),
        r#"package env
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
        r#"package env
env: { B: "true" }
tasks: {
    "build": {
        command: "echo 'b'"
        dependencies: ["c:build"]
    }
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("c")).unwrap();
    fs::write(
        root.join("c/env.cue"),
        r#"package env
env: { C: "true" }
tasks: {
    "build": {
        command: "echo 'c'"
    }
}"#,
    )
    .unwrap();

    // Discover and build registry
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Get execution order
    let executor = TaskExecutor::new_with_registry(registry).await.unwrap();
    let order = executor.get_execution_order("a:build").unwrap();

    // Should be C -> B -> A
    assert_eq!(order, vec!["c:build", "b:build", "a:build"]);
}

/// Test task caching (tasks should only execute once)
#[tokio::test]
async fn test_task_caching() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create a package with diamond dependency: D depends on B and C, both depend on A
    fs::write(
        root.join("env.cue"),
        r#"package env
env: { ROOT: "true" }
tasks: {
    "a": {
        command: "echo 'A' >> executions.txt"
    }
    "b": {
        command: "echo 'B' >> executions.txt"
        dependencies: ["a"]
    }
    "c": {
        command: "echo 'C' >> executions.txt"
        dependencies: ["a"]
    }
    "d": {
        command: "echo 'D' >> executions.txt"
        dependencies: ["b", "c"]
    }
}"#,
    )
    .unwrap();

    // Discover and build registry
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();
    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Execute task D
    let mut executor = TaskExecutor::new_with_registry(registry).await.unwrap();
    executor.execute("root:d").await.unwrap();

    // Check that A was only executed once
    let executions = fs::read_to_string(root.join("executions.txt")).unwrap();
    let a_count = executions.lines().filter(|line| line.contains("A")).count();
    assert_eq!(a_count, 1, "Task A should only execute once");

    // All tasks should be marked as executed
    assert!(executor.is_executed("root:a"));
    assert!(executor.is_executed("root:b"));
    assert!(executor.is_executed("root:c"));
    assert!(executor.is_executed("root:d"));
}
