mod api;
mod builder;
mod cache;
mod context;
mod dependency;
mod execution;
mod graph;
mod management;
mod plan;
mod runner;

pub use context::TaskExecutionContext;
pub use plan::TaskExecutionPlan;

use crate::{MonorepoTaskRegistry, TaskBuilder};
use cuenv_cache::config::CacheConfiguration;
use cuenv_cache::{concurrent::action::ActionCache, CacheManager};
use cuenv_env::manager::EnvManager;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Main task executor that handles dependency resolution and execution
#[derive(Clone)]
pub struct TaskExecutor {
    pub(crate) env_manager: EnvManager,
    pub(crate) working_dir: PathBuf,
    pub(crate) cache_manager: Arc<CacheManager>,
    pub(crate) action_cache: Arc<ActionCache>,
    pub(crate) cache_config: CacheConfiguration,
    /// Task builder for Phase 3 architecture
    pub(crate) task_builder: TaskBuilder,
    /// Optional registry for cross-package task execution in monorepos
    pub(crate) monorepo_registry: Option<Arc<MonorepoTaskRegistry>>,
    /// Track executed tasks to avoid re-execution in cross-package scenarios
    pub(crate) executed_tasks: Arc<Mutex<HashSet<String>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use tempfile::TempDir;

    async fn create_test_env_manager_with_tasks(tasks_cue: &str) -> (EnvManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, tasks_cue).unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).await.unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_simple_task_discovery() {
        let tasks_cue = r#"package env

env: {
    DATABASE_URL: "test"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let tasks = executor.list_tasks();
        assert_eq!(tasks.len(), 2);

        let task_names: Vec<&String> = tasks.iter().map(|(name, _)| name).collect();
        assert!(task_names.contains(&&"build".to_string()));
        assert!(task_names.contains(&&"test".to_string()));
    }

    #[tokio::test]
    async fn test_task_dependency_resolution() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        dependencies: ["test"]
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let plan = executor
            .build_execution_plan(&["build".to_string()])
            .unwrap();

        // Should have 2 levels: [test], [build]
        assert_eq!(plan.levels.len(), 2);
        assert_eq!(plan.levels[0], vec!["test"]);
        assert_eq!(plan.levels[1], vec!["build"]);
    }

    #[tokio::test]
    async fn test_circular_dependency_detection() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "task1": {
        command: "echo 'Task 1'"
        dependencies: ["task2"]
    }
    "task2": {
        command: "echo 'Task 2'"
        dependencies: ["task1"]
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["task1".to_string()]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[tokio::test]
    async fn test_missing_task_error() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        command: "echo 'Building...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["nonexistent".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_missing_dependency_error() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        command: "echo 'Building...'"
        dependencies: ["nonexistent"]
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["build".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_complex_dependency_graph() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "deploy": {
        command: "echo 'Deploying...'"
        dependencies: ["build", "test"]
    }
    "build": {
        command: "echo 'Building...'"
        dependencies: ["compile"]
    }
    "test": {
        command: "echo 'Testing...'"
        dependencies: ["compile"]
    }
    "compile": {
        command: "echo 'Compiling...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = cuenv_cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: cuenv_cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let plan = executor
            .build_execution_plan(&["deploy".to_string()])
            .unwrap();

        // Should have 3 levels: [compile], [build, test], [deploy]
        assert_eq!(plan.levels.len(), 3);
        assert_eq!(plan.levels[0], vec!["compile"]);
        assert_eq!(plan.levels[1].len(), 2);
        assert!(plan.levels[1].contains(&"build".to_string()));
        assert!(plan.levels[1].contains(&"test".to_string()));
        assert_eq!(plan.levels[2], vec!["deploy"]);
    }
}
