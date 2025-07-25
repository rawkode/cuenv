use crate::runtime::{create_runtime_executor, default_runtime_executor};
use crate::cue_parser::{RuntimeConfig, RuntimeType, NixRuntimeConfig, DockerRuntimeConfig};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_host_runtime_execution() {
    let runtime_executor = default_runtime_executor();
    
    assert!(runtime_executor.is_available());
    assert_eq!(runtime_executor.name(), "host");
    
    let env_vars = HashMap::new();
    let temp_dir = TempDir::new().unwrap();
    
    let result = runtime_executor.execute(
        Some("echo 'test'"),
        None,
        Some("sh"),
        temp_dir.path(),
        &env_vars,
        &[],
    ).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_nix_runtime_creation() {
    let runtime_config = RuntimeConfig {
        runtime_type: RuntimeType::Nix,
        config: Some(crate::cue_parser::RuntimeTypeConfig::Nix(NixRuntimeConfig {
            shell: Some("nodejs".to_string()),
            flake: None,
            pure: Some(false),
            args: None,
        })),
    };
    
    let runtime_executor = create_runtime_executor(&runtime_config);
    assert!(runtime_executor.is_ok());
    
    let executor = runtime_executor.unwrap();
    assert_eq!(executor.name(), "nix");
}

#[tokio::test]
async fn test_docker_runtime_creation() {
    let runtime_config = RuntimeConfig {
        runtime_type: RuntimeType::Docker,
        config: Some(crate::cue_parser::RuntimeTypeConfig::Docker(DockerRuntimeConfig {
            image: "alpine:latest".to_string(),
            work_dir: Some("/workspace".to_string()),
            env: None,
            volumes: None,
            network: None,
            args: None,
            rm: Some(true),
        })),
    };
    
    let runtime_executor = create_runtime_executor(&runtime_config);
    assert!(runtime_executor.is_ok());
    
    let executor = runtime_executor.unwrap();
    assert_eq!(executor.name(), "docker");
}

#[tokio::test]
async fn test_runtime_config_validation() {
    // Test that Docker runtime requires image configuration
    let invalid_config = RuntimeConfig {
        runtime_type: RuntimeType::Docker,
        config: None,
    };
    
    let result = create_runtime_executor(&invalid_config);
    assert!(result.is_err());
    
    // Test the error message
    if let Err(e) = result {
        assert!(format!("{}", e).contains("Docker runtime requires image configuration"));
    }
}

#[tokio::test]
async fn test_runtime_environment_variables() {
    let runtime_executor = default_runtime_executor();
    
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
    env_vars.insert("NODE_ENV".to_string(), "development".to_string());
    
    let temp_dir = TempDir::new().unwrap();
    
    let result = runtime_executor.execute(
        Some("env | grep TEST_VAR"),
        None,
        Some("sh"),
        temp_dir.path(),
        &env_vars,
        &[],
    ).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}