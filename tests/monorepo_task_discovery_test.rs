use cuenv::discovery::PackageDiscovery;
use cuenv::task::registry::MonorepoTaskRegistry;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test monorepo structure
fn create_test_monorepo(root: &Path) -> Vec<(&'static str, &'static str)> {
    // Create cue.mod directory
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo""#,
    )
    .unwrap();

    // Create root env.cue with tasks
    fs::write(
        root.join("env.cue"),
        r#"package cuenv

env: {
    ROOT_VAR: "root"
}

tasks: {
    "setup": {
        command: "echo 'Setting up'"
    }
}"#,
    )
    .unwrap();

    // Create frontend package with tasks
    fs::create_dir_all(root.join("projects/frontend")).unwrap();
    fs::write(
        root.join("projects/frontend/env.cue"),
        r#"package cuenv

env: {
    FRONTEND_VAR: "frontend"
}

tasks: {
    "build": {
        command: "vite build"
        outputs: ["dist"]
    }
    "test": {
        command: "vitest"
        dependencies: ["build"]
    }
}"#,
    )
    .unwrap();

    // Create backend package with tasks
    fs::create_dir_all(root.join("projects/backend")).unwrap();
    fs::write(
        root.join("projects/backend/env.cue"),
        r#"package cuenv

env: {
    BACKEND_VAR: "backend"
}

tasks: {
    "build": {
        command: "go build"
        outputs: ["bin/server"]
    }
    "test": {
        command: "go test"
    }
}"#,
    )
    .unwrap();

    // Create CI package with cross-package dependencies
    fs::create_dir_all(root.join("tools/ci")).unwrap();
    fs::write(
        root.join("tools/ci/env.cue"),
        r#"package cuenv

env: {
    CI_VAR: "ci"
}

tasks: {
    "deploy": {
        command: "deploy.sh"
        dependencies: ["projects:frontend:build", "projects:backend:build"]
        inputs: ["projects:frontend:build#dist", "projects:backend:build#bin/server"]
    }
}"#,
    )
    .unwrap();

    vec![
        ("root", "setup"),
        ("projects:frontend", "build"),
        ("projects:frontend", "test"),
        ("projects:backend", "build"),
        ("projects:backend", "test"),
        ("tools:ci", "deploy"),
    ]
}

#[tokio::test]
async fn test_discover_all_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let expected_tasks = create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Count total tasks across all packages
    let mut total_tasks = 0;
    let mut found_tasks = Vec::new();
    for package in &packages {
        if let Some(ref result) = package.parse_result {
            println!("Package {}: {} tasks", package.name, result.tasks.len());
            for task_name in result.tasks.keys() {
                println!("  - {task_name}");
                found_tasks.push((package.name.clone(), task_name.clone()));
            }
            total_tasks += result.tasks.len();
        }
    }

    println!(
        "Total tasks found: {}, expected: {}",
        total_tasks,
        expected_tasks.len()
    );

    // Verify expected tasks are present
    for (package, task) in &expected_tasks {
        let package_str = package.to_string();
        let task_str = task.to_string();
        assert!(
            found_tasks
                .iter()
                .any(|(p, t)| p == &package_str && t == &task_str),
            "Expected task {package}:{task} not found"
        );
    }
}

#[tokio::test]
async fn test_build_task_registry() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Check that all tasks are registered with full names
    assert!(registry.get_task("root:setup").is_some());
    assert!(registry.get_task("projects:frontend:build").is_some());
    assert!(registry.get_task("projects:frontend:test").is_some());
    assert!(registry.get_task("projects:backend:build").is_some());
    assert!(registry.get_task("projects:backend:test").is_some());
    assert!(registry.get_task("tools:ci:deploy").is_some());

    // Non-existent task should return None
    assert!(registry.get_task("nonexistent:task").is_none());
}

#[tokio::test]
async fn test_registry_task_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Check frontend build task
    let frontend_build = registry.get_task("projects:frontend:build").unwrap();
    assert_eq!(frontend_build.package_name, "projects:frontend");
    assert_eq!(frontend_build.task_name, "build");
    assert!(frontend_build.package_path.ends_with("projects/frontend"));
    assert_eq!(
        frontend_build.config.command,
        Some("vite build".to_string())
    );
    assert_eq!(
        frontend_build.config.outputs,
        Some(vec!["dist".to_string()])
    );

    // Check CI deploy task with dependencies
    let ci_deploy = registry.get_task("tools:ci:deploy").unwrap();
    assert_eq!(ci_deploy.package_name, "tools:ci");
    assert_eq!(ci_deploy.task_name, "deploy");
    assert_eq!(
        ci_deploy.config.dependencies,
        Some(vec![
            "projects:frontend:build".to_string(),
            "projects:backend:build".to_string()
        ])
    );
}

#[tokio::test]
async fn test_list_all_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();
    let all_tasks = registry.list_all_tasks();

    assert!(
        all_tasks.len() >= 6,
        "Should have at least 6 tasks, found {}",
        all_tasks.len()
    );

    // Check that task names are properly formatted
    let task_names: Vec<String> = all_tasks.iter().map(|t| t.0.clone()).collect();
    assert!(task_names.contains(&"root:setup".to_string()));
    assert!(task_names.contains(&"projects:frontend:build".to_string()));
    assert!(task_names.contains(&"tools:ci:deploy".to_string()));
}

#[tokio::test]
async fn test_get_tasks_by_package() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Get tasks for frontend package
    let frontend_tasks = registry.get_tasks_by_package("projects:frontend");
    assert!(
        frontend_tasks.len() >= 2,
        "Frontend should have at least 2 tasks, found {}",
        frontend_tasks.len()
    );

    let task_names: Vec<&str> = frontend_tasks
        .iter()
        .map(|t| t.task_name.as_str())
        .collect();
    assert!(task_names.contains(&"build"));
    assert!(task_names.contains(&"test"));

    // Get tasks for non-existent package
    let empty = registry.get_tasks_by_package("nonexistent");
    assert_eq!(empty.len(), 0);
}

#[tokio::test]
async fn test_resolve_task_outputs() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    // Create actual output directories to test resolution
    fs::create_dir_all(root.join("projects/frontend/dist")).unwrap();
    fs::write(root.join("projects/frontend/dist/index.html"), "test").unwrap();

    fs::create_dir_all(root.join("projects/backend/bin")).unwrap();
    fs::write(root.join("projects/backend/bin/server"), "binary").unwrap();

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Resolve frontend build output
    let frontend_dist = registry
        .resolve_task_output("projects:frontend:build", "dist")
        .unwrap();
    assert!(frontend_dist.exists());
    assert!(frontend_dist.is_dir());
    assert!(frontend_dist.join("index.html").exists());

    // Resolve backend build output
    let backend_bin = registry
        .resolve_task_output("projects:backend:build", "bin/server")
        .unwrap();
    assert!(backend_bin.exists());
    assert!(backend_bin.is_file());

    // Non-existent task
    assert!(registry
        .resolve_task_output("nonexistent:task", "output")
        .is_err());

    // Non-existent output
    assert!(registry
        .resolve_task_output("projects:frontend:build", "nonexistent")
        .is_err());
}

#[tokio::test]
async fn test_registry_with_missing_package() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create minimal monorepo
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/minimal""#,
    )
    .unwrap();

    // Create one package with dependency on non-existent package
    fs::write(
        root.join("env.cue"),
        r#"package cuenv

tasks: {
    "deploy": {
        command: "deploy"
        dependencies: ["nonexistent:package:task"]
    }
}"#,
    )
    .unwrap();

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Debug: print package info
    println!("Found {} packages", packages.len());
    for package in &packages {
        println!(
            "Package {}: parse_result = {:?}",
            package.name,
            package.parse_result.is_some()
        );
        if let Some(ref result) = package.parse_result {
            println!("  Tasks: {:?}", result.tasks.keys().collect::<Vec<_>>());
            println!("  Num tasks: {}", result.tasks.len());
        }
    }

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Debug: print all tasks
    let all_tasks = registry.list_all_tasks();
    println!("All tasks in registry: {all_tasks:?}");

    // The registry should be created successfully
    assert!(
        registry.get_task("root:deploy").is_some(),
        "Task 'root:deploy' not found. Available tasks: {all_tasks:?}"
    );

    // But validation of dependencies should fail
    let validation = registry.validate_all_dependencies();
    assert!(validation.is_err());
    assert!(validation
        .unwrap_err()
        .to_string()
        .contains("nonexistent:package:task"));
}

#[tokio::test]
async fn test_registry_package_paths() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    let registry = MonorepoTaskRegistry::from_packages(packages).unwrap();

    // Get package path for a task
    let frontend_build = registry.get_task("projects:frontend:build").unwrap();
    assert!(frontend_build.package_path.ends_with("projects/frontend"));
    assert!(frontend_build.package_path.join("env.cue").exists());

    // Package paths should be absolute
    assert!(frontend_build.package_path.is_absolute());
}
