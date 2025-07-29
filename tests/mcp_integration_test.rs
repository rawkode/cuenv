#![allow(unused)]
use cuenv::mcp::tools::CuenvToolBox;
use cuenv::mcp::types::*;
use std::fs;
use tempfile::TempDir;
use tokio;

// Helper function to create test environment
fn create_test_env(temp_dir: &TempDir) -> String {
    let env_file = temp_dir.path().join("env.cue");

    fs::write(
        &env_file,
        r#"package env

env: {
    APP_NAME: "test-app"
    VERSION: "1.0.0"
    DEBUG: false
}

environments: {
    dev: {
        DEBUG: true
        PORT: 3000
    }
    staging: {
        DEBUG: false
        PORT: 4000
    }
    production: {
        DEBUG: false
        PORT: 8080
    }
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        capabilities: ["build"]
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
        dependencies: ["build"]
        capabilities: ["test"]
    }
    "deploy": {
        description: "Deploy to production"
        script: "echo 'Deploying...'"
        dependencies: ["test"]
        capabilities: ["deploy"]
    }
}

commands: {
    "lint": {
        capabilities: ["lint"]
    }
}

metadata: {
    APP_NAME: {
        capability: "app"
    }
    DEBUG: {
        capability: "debug"
    }
}
"#,
    )
    .unwrap();

    temp_dir.path().to_string_lossy().to_string()
}

#[tokio::test]
async fn test_mcp_list_env_vars() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = EnvVarParams {
        directory: directory.clone(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.list_env_vars(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.variables.contains_key("APP_NAME"));
    assert_eq!(response.variables.get("APP_NAME").unwrap(), "test-app");
    assert!(response.variables.contains_key("VERSION"));
    assert_eq!(response.variables.get("VERSION").unwrap(), "1.0.0");
}

#[tokio::test]
async fn test_mcp_get_env_var() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = GetEnvVarParams {
        directory: directory.clone(),
        name: "APP_NAME".to_string(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.get_env_var(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response, Some("test-app".to_string()));
}

#[tokio::test]
async fn test_mcp_get_env_var_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = GetEnvVarParams {
        directory: directory.clone(),
        name: "NONEXISTENT_VAR".to_string(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.get_env_var(params).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[tokio::test]
async fn test_mcp_list_environments() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = DirectoryParams {
        directory: directory.clone(),
    };

    let result = toolbox.list_environments(params).await;
    assert!(result.is_ok());

    let environments = result.unwrap();
    assert!(environments.contains(&"dev".to_string()));
    assert!(environments.contains(&"staging".to_string()));
    assert!(environments.contains(&"production".to_string()));
    assert_eq!(environments.len(), 3);
}

#[tokio::test]
async fn test_mcp_list_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = TaskParams {
        directory: directory.clone(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.list_tasks(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.tasks.len(), 3);

    let task_names: Vec<String> = response.tasks.iter().map(|t| t.name.clone()).collect();
    assert!(task_names.contains(&"build".to_string()));
    assert!(task_names.contains(&"test".to_string()));
    assert!(task_names.contains(&"deploy".to_string()));
}

#[tokio::test]
async fn test_mcp_get_task() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = GetTaskParams {
        directory: directory.clone(),
        name: "build".to_string(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.get_task(params).await;
    assert!(result.is_ok());

    let task = result.unwrap();
    assert!(task.is_some());

    let task_info = task.unwrap();
    assert_eq!(task_info.name, "build");
    assert_eq!(task_info.description, Some("Build the project".to_string()));
    assert_eq!(task_info.command, Some("echo 'Building...'".to_string()));
}

#[tokio::test]
async fn test_mcp_get_task_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = GetTaskParams {
        directory: directory.clone(),
        name: "nonexistent".to_string(),
        environment: None,
        capabilities: None,
    };

    let result = toolbox.get_task(params).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_mcp_run_task_without_allow_exec() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = RunTaskParams {
        directory: directory.clone(),
        name: "build".to_string(),
        args: None,
        environment: None,
        capabilities: None,
    };

    let result = toolbox.run_task(params).await;
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("Task execution not allowed"));
}

#[tokio::test]
async fn test_mcp_run_task_with_allow_exec() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: true };

    let params = RunTaskParams {
        directory: directory.clone(),
        name: "build".to_string(),
        args: None,
        environment: None,
        capabilities: None,
    };

    let result = toolbox.run_task(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.exit_code, 0);
    assert!(response.success);
}

#[tokio::test]
async fn test_mcp_check_directory_allowed() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = DirectoryParams {
        directory: directory.clone(),
    };

    let result = toolbox.check_directory(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.allowed);
    assert!(response.has_env_cue);
}

#[tokio::test]
async fn test_mcp_check_directory_not_allowed() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Don't allow the directory

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = DirectoryParams {
        directory: directory.clone(),
    };

    let result = toolbox.check_directory(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(!response.allowed);
    assert!(response.has_env_cue);
}

#[tokio::test]
async fn test_mcp_list_capabilities() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = DirectoryParams {
        directory: directory.clone(),
    };

    let result = toolbox.list_capabilities(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let caps = response.capabilities;

    // Should contain capabilities from tasks, commands, and metadata
    assert!(caps.contains(&"build".to_string()));
    assert!(caps.contains(&"test".to_string()));
    assert!(caps.contains(&"deploy".to_string()));
    assert!(caps.contains(&"lint".to_string()));
    assert!(caps.contains(&"app".to_string()));
    assert!(caps.contains(&"debug".to_string()));
}

#[tokio::test]
async fn test_mcp_directory_validation_nonexistent() {
    let toolbox = CuenvToolBox { allow_exec: false };

    let params = DirectoryParams {
        directory: "/nonexistent/directory".to_string(),
    };

    let result = toolbox.check_directory(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(!response.allowed);
    assert!(!response.has_env_cue);
}

#[tokio::test]
async fn test_mcp_directory_validation_path_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    let toolbox = CuenvToolBox { allow_exec: false };

    // Try path traversal attack
    let malicious_path = format!("{}/../", directory);
    let params = DirectoryParams {
        directory: malicious_path,
    };

    let result = toolbox
        .list_env_vars(EnvVarParams {
            directory: params.directory.clone(),
            environment: None,
            capabilities: None,
        })
        .await;

    // Should fail due to canonicalization and permission checks
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_env_var_with_environment() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = EnvVarParams {
        directory: directory.clone(),
        environment: Some("dev".to_string()),
        capabilities: None,
    };

    let result = toolbox.list_env_vars(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    // In dev environment, DEBUG should be true and PORT should be 3000
    assert_eq!(response.variables.get("DEBUG"), Some(&"true".to_string()));
    assert_eq!(response.variables.get("PORT"), Some(&"3000".to_string()));
}

#[tokio::test]
async fn test_mcp_env_var_with_capabilities() {
    let temp_dir = TempDir::new().unwrap();
    let directory = create_test_env(&temp_dir);

    // Allow the directory first
    let _ = std::process::Command::new("cargo")
        .args(&["run", "--", "allow", &directory])
        .env("XDG_CACHE_HOME", temp_dir.path().join(".cache"))
        .output();

    let toolbox = CuenvToolBox { allow_exec: false };

    let params = EnvVarParams {
        directory: directory.clone(),
        environment: None,
        capabilities: Some(vec!["app".to_string()]),
    };

    let result = toolbox.list_env_vars(params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    // Should only contain variables with "app" capability
    assert!(response.variables.contains_key("APP_NAME"));
    // DEBUG has "debug" capability, so it might not be included
}
