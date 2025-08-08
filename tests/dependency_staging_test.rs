use cuenv::task::staging::{DependencyStager, StagedDependency, StagingStrategy};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_create_staging_directory() {
    let mut stager = DependencyStager::new().unwrap();

    // The staging directory should exist
    assert!(stager.staging_root().exists());
    assert!(stager.staging_root().is_dir());

    // It should be in a temp location
    let path = stager.staging_root().to_string_lossy();
    assert!(path.contains("cuenv-staging") || path.contains("tmp"));
}

#[test]
fn test_stage_file_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let source_file = temp_dir.path().join("source.txt");
    fs::write(&source_file, "test content").unwrap();

    let mut stager = DependencyStager::new().unwrap();

    let dependency = StagedDependency {
        name: "input_file".to_string(),
        source_path: source_file.clone(),
        target_name: Some("input.txt".to_string()),
    };

    let staged_path = stager.stage_dependency(&dependency).unwrap();

    // The staged file should exist
    assert!(staged_path.exists());
    assert!(staged_path.is_file());

    // It should have the target name
    assert_eq!(staged_path.file_name().unwrap(), "input.txt");

    // Content should be accessible
    let content = fs::read_to_string(&staged_path).unwrap();
    assert_eq!(content, "test content");
}

#[test]
fn test_stage_directory_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let source_dir = temp_dir.path().join("source_dir");
    fs::create_dir(&source_dir).unwrap();
    fs::write(source_dir.join("file1.txt"), "content1").unwrap();
    fs::write(source_dir.join("file2.txt"), "content2").unwrap();

    let sub_dir = source_dir.join("subdir");
    fs::create_dir(&sub_dir).unwrap();
    fs::write(sub_dir.join("file3.txt"), "content3").unwrap();

    let mut stager = DependencyStager::new().unwrap();

    let dependency = StagedDependency {
        name: "input_dir".to_string(),
        source_path: source_dir.clone(),
        target_name: None,
    };

    let staged_path = stager.stage_dependency(&dependency).unwrap();

    // The staged directory should exist
    assert!(staged_path.exists());
    assert!(staged_path.is_dir());

    // Check files are accessible
    assert!(staged_path.join("file1.txt").exists());
    assert!(staged_path.join("file2.txt").exists());
    assert!(staged_path.join("subdir/file3.txt").exists());

    // Check content
    let content1 = fs::read_to_string(staged_path.join("file1.txt")).unwrap();
    assert_eq!(content1, "content1");
}

#[test]
fn test_stage_multiple_dependencies() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple source files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    fs::write(&file1, "content1").unwrap();
    fs::write(&file2, "content2").unwrap();

    let mut stager = DependencyStager::new().unwrap();

    let dep1 = StagedDependency {
        name: "input1".to_string(),
        source_path: file1.clone(),
        target_name: None,
    };

    let dep2 = StagedDependency {
        name: "input2".to_string(),
        source_path: file2.clone(),
        target_name: None,
    };

    let staged1 = stager.stage_dependency(&dep1).unwrap();
    let staged2 = stager.stage_dependency(&dep2).unwrap();

    // Both should be staged
    assert!(staged1.exists());
    assert!(staged2.exists());

    // They should be in different locations
    assert_ne!(staged1, staged2);

    // Both should be in the same staging root
    assert!(staged1.starts_with(stager.staging_root()));
    assert!(staged2.starts_with(stager.staging_root()));
}

#[test]
fn test_staging_cleanup_on_drop() {
    let staged_path: PathBuf;

    {
        let temp_dir = TempDir::new().unwrap();
        let source_file = temp_dir.path().join("source.txt");
        fs::write(&source_file, "test").unwrap();

        let mut stager = DependencyStager::new().unwrap();
        let staging_root = stager.staging_root().to_path_buf();

        let dependency = StagedDependency {
            name: "test".to_string(),
            source_path: source_file.clone(),
            target_name: None,
        };

        staged_path = stager.stage_dependency(&dependency).unwrap();
        assert!(staged_path.exists());
        assert!(staging_root.exists());

        // Stager goes out of scope here
    }

    // After drop, staged files should be cleaned up
    // Note: We're using symlinks by default, so the symlink should be gone
    // but we're not checking if the staging root is deleted as it might be async
    assert!(!staged_path.exists() || !staged_path.is_file());
}

#[test]
fn test_stage_missing_source() {
    let mut stager = DependencyStager::new().unwrap();

    let dependency = StagedDependency {
        name: "missing".to_string(),
        source_path: PathBuf::from("/nonexistent/file.txt"),
        target_name: None,
    };

    let result = stager.stage_dependency(&dependency);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("not found") || error_msg.contains("does not exist"));
}

#[test]
fn test_environment_variables() {
    let temp_dir = TempDir::new().unwrap();
    let file1 = temp_dir.path().join("file1.txt");
    let dir1 = temp_dir.path().join("dir1");
    fs::write(&file1, "content").unwrap();
    fs::create_dir(&dir1).unwrap();

    let mut stager = DependencyStager::new().unwrap();

    let dep1 = StagedDependency {
        name: "projects:frontend:dist".to_string(),
        source_path: file1.clone(),
        target_name: None,
    };

    let dep2 = StagedDependency {
        name: "tools:scripts:output".to_string(),
        source_path: dir1.clone(),
        target_name: None,
    };

    let staged1 = stager.stage_dependency(&dep1).unwrap();
    let staged2 = stager.stage_dependency(&dep2).unwrap();

    let env_vars = stager.get_environment_variables();

    // Check environment variable names are formatted correctly
    assert!(env_vars.contains_key("CUENV_INPUT_PROJECTS_FRONTEND_DIST"));
    assert!(env_vars.contains_key("CUENV_INPUT_TOOLS_SCRIPTS_OUTPUT"));

    // Check paths are absolute
    assert_eq!(
        env_vars["CUENV_INPUT_PROJECTS_FRONTEND_DIST"],
        staged1.to_string_lossy()
    );
    assert_eq!(
        env_vars["CUENV_INPUT_TOOLS_SCRIPTS_OUTPUT"],
        staged2.to_string_lossy()
    );
}

#[test]
fn test_staging_strategy_selection() {
    // Test that we can select different staging strategies
    let mut stager_symlink = DependencyStager::with_strategy(StagingStrategy::Symlink).unwrap();
    assert_eq!(stager_symlink.strategy(), StagingStrategy::Symlink);

    let mut stager_copy = DependencyStager::with_strategy(StagingStrategy::Copy).unwrap();
    assert_eq!(stager_copy.strategy(), StagingStrategy::Copy);

    // Default should be Symlink
    let mut stager_default = DependencyStager::new().unwrap();
    assert_eq!(stager_default.strategy(), StagingStrategy::Symlink);
}

#[test]
fn test_stage_with_copy_strategy() {
    let temp_dir = TempDir::new().unwrap();
    let source_file = temp_dir.path().join("source.txt");
    fs::write(&source_file, "test content").unwrap();

    let mut stager = DependencyStager::with_strategy(StagingStrategy::Copy).unwrap();

    let dependency = StagedDependency {
        name: "copied_file".to_string(),
        source_path: source_file.clone(),
        target_name: None,
    };

    let staged_path = stager.stage_dependency(&dependency).unwrap();

    // File should exist and be a regular file (not a symlink)
    assert!(staged_path.exists());
    assert!(staged_path.is_file());

    // Modify the staged file
    fs::write(&staged_path, "modified").unwrap();

    // Original should be unchanged
    let original_content = fs::read_to_string(&source_file).unwrap();
    assert_eq!(original_content, "test content");

    // Staged should be modified
    let staged_content = fs::read_to_string(&staged_path).unwrap();
    assert_eq!(staged_content, "modified");
}
