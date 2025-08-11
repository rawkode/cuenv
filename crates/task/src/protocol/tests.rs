//! Tests for the protocol module

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_task_server_client_creation() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let client = TaskServerClient::new(socket_path.clone());
        assert_eq!(client.socket_path, socket_path);
    }

    #[tokio::test]
    async fn test_task_server_manager_creation() {
        let temp_dir = TempDir::new().unwrap();

        let manager = TaskServerManager::new(temp_dir.path().to_path_buf());
        assert_eq!(manager.socket_dir, temp_dir.path());
    }

    #[tokio::test]
    async fn test_discover_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let socket_dir = temp_dir.path().join("sockets");
        fs::create_dir_all(&socket_dir).unwrap();

        let mut manager = TaskServerManager::new(socket_dir);
        let tasks = manager.discover_servers(temp_dir.path()).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_task_server_provider_creation() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("provider.sock");

        // Create a mock config with tasks
        let mut tasks = HashMap::new();
        tasks.insert(
            "test".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Test task".to_string()),
                command: Some("echo hello".to_string()),
                ..Default::default()
            },
        );

        use cuenv_config::{ParseResult, RuntimeOptions};
        let parse_result = ParseResult {
            variables: HashMap::new(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks,
            hooks: HashMap::new(),
        };
        let config = Arc::new(cuenv_config::Config::new(
            temp_dir.path().to_path_buf(),
            None,
            parse_result,
            RuntimeOptions::default(),
        ));

        let provider = TaskServerProvider::new(socket_path.clone(), config);
        assert_eq!(provider.socket_path, Some(socket_path));
        assert!(provider.listener.is_none());
    }

    #[tokio::test]
    async fn test_unified_task_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "test".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Test task".to_string()),
                ..Default::default()
            },
        );

        let parse_result = cuenv_config::ParseResult {
            variables: HashMap::new(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks: tasks.clone(),
            hooks: HashMap::new(),
        };
        let config = Arc::new(cuenv_config::Config::new(
            temp_dir.path().to_path_buf(),
            None,
            parse_result,
            cuenv_config::RuntimeOptions::default(),
        ));
        let manager = UnifiedTaskManager::new(temp_dir.path().to_path_buf(), config.clone());
        assert_eq!(manager.config.get_tasks(), &tasks);
        assert!(manager.server_provider.is_none());
    }

    #[test]
    fn test_export_tasks_to_json() {
        let temp_dir = TempDir::new().unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "build".to_string(),
            cuenv_config::TaskConfig {
                description: Some("Build the project".to_string()),
                dependencies: Some(vec!["deps".to_string()]),
                ..Default::default()
            },
        );

        let parse_result = cuenv_config::ParseResult {
            variables: HashMap::new(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks,
            hooks: HashMap::new(),
        };
        let config = Arc::new(cuenv_config::Config::new(
            temp_dir.path().to_path_buf(),
            None,
            parse_result,
            cuenv_config::RuntimeOptions::default(),
        ));
        let manager = UnifiedTaskManager::new(temp_dir.path().to_path_buf(), config);
        let json = manager.export_tasks_to_json().unwrap();

        // Verify JSON contains expected structure
        assert!(json.contains("tasks"));
        assert!(json.contains("build"));
        assert!(json.contains("Build the project"));
        assert!(json.contains("deps"));
    }
}
