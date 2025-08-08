use cuenv::discovery::PackageDiscovery;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test monorepo structure
fn create_test_monorepo(root: &Path) {
    // Create cue.mod directory
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/monorepo"
language: {
    version: "v0.13.2"
}"#,
    )
    .unwrap();

    // Create root env.cue
    fs::write(
        root.join("env.cue"),
        r#"package env

env: {
    ROOT_VAR: "root_value"
}"#,
    )
    .unwrap();

    // Create projects structure
    fs::create_dir_all(root.join("projects/frontend")).unwrap();
    fs::write(
        root.join("projects/frontend/env.cue"),
        r#"package env

env: {
    FRONTEND_VAR: "frontend_value"
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("projects/backend")).unwrap();
    fs::write(
        root.join("projects/backend/env.cue"),
        r#"package env

env: {
    BACKEND_VAR: "backend_value"
}"#,
    )
    .unwrap();

    // Create tools structure
    fs::create_dir_all(root.join("tools/ci")).unwrap();
    fs::write(
        root.join("tools/ci/env.cue"),
        r#"package env

env: {
    CI_VAR: "ci_value"
}"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("tools/scripts")).unwrap();
    fs::write(
        root.join("tools/scripts/env.cue"),
        r#"package env

env: {
    SCRIPTS_VAR: "scripts_value"
}"#,
    )
    .unwrap();

    // Create a deeply nested structure to test max depth
    let mut deep_path = root.to_path_buf();
    for i in 0..35 {
        deep_path = deep_path.join(format!("level{i}"));
        fs::create_dir_all(&deep_path).unwrap();
    }
    fs::write(
        deep_path.join("env.cue"),
        r#"package env

env: {
    DEEP_VAR: "deep_value"
}"#,
    )
    .unwrap();
}

#[test]
fn test_find_module_root() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create cue.mod directory
    fs::create_dir_all(root.join("cue.mod")).unwrap();

    // Create nested directories
    let nested = root.join("a/b/c/d");
    fs::create_dir_all(&nested).unwrap();

    // Test finding from various locations
    assert_eq!(
        PackageDiscovery::find_module_root(root).unwrap(),
        root.to_path_buf()
    );

    assert_eq!(
        PackageDiscovery::find_module_root(&nested).unwrap(),
        root.to_path_buf()
    );

    // Test when no cue.mod exists
    let no_module = TempDir::new().unwrap();
    assert!(PackageDiscovery::find_module_root(no_module.path()).is_err());
}

#[tokio::test]
async fn test_discover_env_files() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let env_files = discovery.discover_env_files(root).unwrap();

    // Should find 5 env.cue files (root + 4 in subdirs)
    // The deeply nested one should be ignored due to max_depth
    assert_eq!(env_files.len(), 5);

    // Verify specific files are found
    let file_names: Vec<String> = env_files
        .iter()
        .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
        .collect();

    assert!(file_names.contains(&"env.cue".to_string()));
    assert!(file_names.contains(&format!(
        "projects{}frontend{}env.cue",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    )));
    assert!(file_names.contains(&format!(
        "projects{}backend{}env.cue",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    )));
    assert!(file_names.contains(&format!(
        "tools{}ci{}env.cue",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    )));
    assert!(file_names.contains(&format!(
        "tools{}scripts{}env.cue",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    )));
}

#[test]
fn test_format_package_name() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join("cue.mod")).unwrap();

    let mut discovery = PackageDiscovery::new(32);
    // Manually set module_root for testing
    discovery.module_root = Some(root.to_path_buf());

    // Test root package
    assert_eq!(
        discovery
            .format_package_name(&root.join("env.cue"))
            .unwrap(),
        "root"
    );

    // Test nested packages
    assert_eq!(
        discovery
            .format_package_name(&root.join("projects/frontend/env.cue"))
            .unwrap(),
        "projects:frontend"
    );

    assert_eq!(
        discovery
            .format_package_name(&root.join("tools/ci/env.cue"))
            .unwrap(),
        "tools:ci"
    );

    // Test deeply nested
    assert_eq!(
        discovery
            .format_package_name(&root.join("a/b/c/d/env.cue"))
            .unwrap(),
        "a:b:c:d"
    );
}

#[tokio::test]
async fn test_discover_packages_without_loading() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, false).await.unwrap();

    assert_eq!(packages.len(), 5);

    // Check package names
    let names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"root".to_string()));
    assert!(names.contains(&"projects:frontend".to_string()));
    assert!(names.contains(&"projects:backend".to_string()));
    assert!(names.contains(&"tools:ci".to_string()));
    assert!(names.contains(&"tools:scripts".to_string()));

    // Since we didn't load, parse_result should be None
    for package in &packages {
        assert!(package.parse_result.is_none());
    }
}

#[tokio::test]
async fn test_discover_packages_with_loading() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    assert_eq!(packages.len(), 5);

    // All packages should be loaded
    for package in &packages {
        assert!(package.parse_result.is_some());

        // Verify some content from each package
        if let Some(ref result) = package.parse_result {
            match package.name.as_str() {
                "root" => {
                    assert!(result.variables.contains_key("ROOT_VAR"));
                    assert_eq!(result.variables.get("ROOT_VAR").unwrap(), "root_value");
                }
                "projects:frontend" => {
                    assert!(result.variables.contains_key("FRONTEND_VAR"));
                    assert_eq!(
                        result.variables.get("FRONTEND_VAR").unwrap(),
                        "frontend_value"
                    );
                }
                "projects:backend" => {
                    assert!(result.variables.contains_key("BACKEND_VAR"));
                    assert_eq!(
                        result.variables.get("BACKEND_VAR").unwrap(),
                        "backend_value"
                    );
                }
                "tools:ci" => {
                    assert!(result.variables.contains_key("CI_VAR"));
                    assert_eq!(result.variables.get("CI_VAR").unwrap(), "ci_value");
                }
                "tools:scripts" => {
                    assert!(result.variables.contains_key("SCRIPTS_VAR"));
                    assert_eq!(
                        result.variables.get("SCRIPTS_VAR").unwrap(),
                        "scripts_value"
                    );
                }
                _ => panic!("Unexpected package name: {}", package.name),
            }
        }
    }
}

#[tokio::test]
async fn test_load_package_by_name() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    let mut discovery = PackageDiscovery::new(32);

    // Test loading specific package
    let package = discovery
        .load_package_by_name(root, "projects:backend")
        .await
        .unwrap();

    assert_eq!(package.name, "projects:backend");
    assert!(package.parse_result.is_some());

    let result = package.parse_result.unwrap();
    assert!(result.variables.contains_key("BACKEND_VAR"));
    assert_eq!(
        result.variables.get("BACKEND_VAR").unwrap(),
        "backend_value"
    );

    // Test loading non-existent package
    assert!(discovery
        .load_package_by_name(root, "nonexistent:package")
        .await
        .is_err());
}

#[tokio::test]
async fn test_max_depth_limiting() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    // Test with different max depths
    let mut discovery5 = PackageDiscovery::new(5);
    let packages5 = discovery5.discover(root, false).await.unwrap();

    // Should find all packages at depth <= 5
    assert_eq!(packages5.len(), 5);

    // Test with very small max depth
    let mut discovery1 = PackageDiscovery::new(1);
    let packages1 = discovery1.discover(root, false).await.unwrap();

    // Should only find root env.cue
    assert_eq!(packages1.len(), 1);
    assert_eq!(packages1[0].name, "root");
}

#[tokio::test]
async fn test_discover_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_monorepo(root);

    // Start discovery from a subdirectory
    let start_path = root.join("projects/frontend");

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(&start_path, false).await.unwrap();

    // Should still find all packages because it searches from module root
    assert_eq!(packages.len(), 5);
}

#[tokio::test]
async fn test_empty_monorepo() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create only cue.mod, no env.cue files
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/empty""#,
    )
    .unwrap();

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, false).await.unwrap();

    assert_eq!(packages.len(), 0);
}

#[tokio::test]
async fn test_discovery_with_invalid_cue_files() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create cue.mod
    fs::create_dir_all(root.join("cue.mod")).unwrap();
    fs::write(
        root.join("cue.mod/module.cue"),
        r#"module: "test.example/invalid""#,
    )
    .unwrap();

    // Create valid env.cue
    fs::write(
        root.join("env.cue"),
        r#"package env
env: {
    VALID: "yes"
}"#,
    )
    .unwrap();

    // Create invalid env.cue (syntax error)
    fs::create_dir_all(root.join("broken")).unwrap();
    fs::write(
        root.join("broken/env.cue"),
        r#"package env
env: {
    INVALID: "missing closing brace"
"#,
    )
    .unwrap();

    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(root, true).await.unwrap();

    // Should discover both files
    assert_eq!(packages.len(), 2);

    // Valid package should load successfully
    let valid_package = packages.iter().find(|p| p.name == "root").unwrap();
    assert!(valid_package.parse_result.is_some());

    // Invalid package should fail to load but still be discovered
    let invalid_package = packages.iter().find(|p| p.name == "broken").unwrap();
    assert!(invalid_package.parse_result.is_none());
}
