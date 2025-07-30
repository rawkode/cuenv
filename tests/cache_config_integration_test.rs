//! Integration tests for cache configuration system with real CUE files
use cuenv::cache::{CacheConfigLoader, CacheConfigResolver};
use cuenv::cue_parser::{CueParser, ParseOptions};
use cuenv::task_executor::TaskExecutor;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_cache_config_with_simple_cue_file() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        cache: true
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
        cache: false
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Parse the CUE file
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &ParseOptions::default())
            .unwrap();

    // Check that tasks are parsed correctly
    assert!(result.tasks.contains_key("build"));
    assert!(result.tasks.contains_key("test"));

    let build_task = &result.tasks["build"];
    let test_task = &result.tasks["test"];

    // Check cache configuration
    assert_eq!(
        build_task.cache,
        Some(cuenv::cache::TaskCacheConfig::Simple(true))
    );
    assert_eq!(
        test_task.cache,
        Some(cuenv::cache::TaskCacheConfig::Simple(false))
    );

    // Test cache decision logic
    let cache_config = CacheConfigLoader::load().unwrap();

    let build_should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        build_task.cache.as_ref(),
        "build",
    );

    let test_should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        test_task.cache.as_ref(),
        "test",
    );

    assert!(build_should_cache);
    assert!(!test_should_cache);
}

#[tokio::test]
async fn test_cache_config_with_advanced_cue_file() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "deploy": {
        description: "Deploy the project"
        command: "echo 'Deploying...'"
        cache: {
            enabled: true
            env: {
                include: ["DEPLOY_*", "AWS_*"]
                exclude: ["DEPLOY_DEBUG"]
                useSmartDefaults: true
            }
        }
    }
    "cleanup": {
        description: "Clean up resources"
        command: "echo 'Cleaning up...'"
        cache: {
            enabled: false
        }
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Parse the CUE file
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &ParseOptions::default())
            .unwrap();

    // Check that tasks are parsed correctly
    assert!(result.tasks.contains_key("deploy"));
    assert!(result.tasks.contains_key("cleanup"));

    let deploy_task = &result.tasks["deploy"];
    let cleanup_task = &result.tasks["cleanup"];

    // Check cache configuration
    assert!(matches!(
        deploy_task.cache,
        Some(cuenv::cache::TaskCacheConfig::Advanced {
            enabled: true,
            env: Some(_)
        })
    ));

    assert!(matches!(
        cleanup_task.cache,
        Some(cuenv::cache::TaskCacheConfig::Advanced {
            enabled: false,
            env: None
        })
    ));

    // Test cache decision logic
    let cache_config = CacheConfigLoader::load().unwrap();

    let deploy_should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        deploy_task.cache.as_ref(),
        "deploy",
    );

    let cleanup_should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        cleanup_task.cache.as_ref(),
        "cleanup",
    );

    assert!(deploy_should_cache);
    assert!(!cleanup_should_cache);
}

#[tokio::test]
async fn test_cache_config_with_global_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        cache: true
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Set environment variable to disable caching globally
    std::env::set_var("CUENV_CACHE_ENABLED", "false");

    // Parse the CUE file
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &ParseOptions::default())
            .unwrap();
    let build_task = &result.tasks["build"];

    // Test cache decision logic
    let cache_config = CacheConfigLoader::load().unwrap();

    let should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        build_task.cache.as_ref(),
        "build",
    );

    // Global disabled should override task enabled
    assert!(!should_cache);

    // Clean up
    std::env::remove_var("CUENV_CACHE_ENABLED");
}

#[tokio::test]
async fn test_cache_config_with_no_task_config() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        // No cache configuration - should use global default
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Parse the CUE file
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &ParseOptions::default())
            .unwrap();
    let build_task = &result.tasks["build"];

    // Check that no cache configuration is present
    assert!(build_task.cache.is_none());

    // Test cache decision logic
    let cache_config = CacheConfigLoader::load().unwrap();

    let should_cache = CacheConfigResolver::should_cache_task(
        &cache_config.global,
        build_task.cache.as_ref(),
        "build",
    );

    // Should use global default (enabled)
    assert!(should_cache);
}

#[tokio::test]
async fn test_cache_config_with_task_executor() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "cached-task": {
        description: "Task with caching enabled"
        command: "echo 'Cached task'"
        cache: true
    }
    "uncached-task": {
        description: "Task with caching disabled"
        command: "echo 'Uncached task'"
        cache: false
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Create environment manager
    let mut env_manager = cuenv::env_manager::EnvManager::new();
    env_manager.load_env(temp_dir.path()).await.unwrap();

    // Create task executor
    let executor = TaskExecutor::new(env_manager, temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // List tasks to verify they were loaded
    let tasks = executor.list_tasks();
    assert_eq!(tasks.len(), 2);

    let task_names: Vec<&String> = tasks.iter().map(|(name, _)| name).collect();
    assert!(task_names.contains(&&"cached-task".to_string()));
    assert!(task_names.contains(&&"uncached-task".to_string()));

    // Test cache statistics
    let stats = executor.get_cache_statistics().unwrap();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.writes, 0);
}

#[tokio::test]
async fn test_cache_config_with_env_vars() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let cue_content = r#"
package env

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        cache: true
    }
}
"#;

    fs::write(&env_file, cue_content).unwrap();

    // Set environment variables for cache configuration
    std::env::set_var("CUENV_CACHE", "read");
    std::env::set_var("CUENV_CACHE_ENABLED", "true");
    std::env::set_var("CUENV_CACHE_MAX_SIZE", "1048576"); // 1MB

    // Load cache configuration
    let cache_config = CacheConfigLoader::load().unwrap();

    // Verify environment variables were applied
    assert_eq!(cache_config.global.mode, cuenv::cache::CacheMode::Read);
    assert!(cache_config.global.enabled);
    assert_eq!(cache_config.global.max_size, Some(1048576));

    // Clean up
    std::env::remove_var("CUENV_CACHE");
    std::env::remove_var("CUENV_CACHE_ENABLED");
    std::env::remove_var("CUENV_CACHE_MAX_SIZE");
}

#[tokio::test]
async fn test_cache_config_with_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("cuenv");
    std::fs::create_dir_all(&config_dir).unwrap();

    let config_file = config_dir.join("config.json");
    let config_content = r#"
    {
        "cache": {
            "enabled": false,
            "mode": "read",
            "max_size": 5242880,
            "inline_threshold": 2048
        }
    }
    "#;

    std::fs::write(&config_file, config_content).unwrap();

    // Set XDG_CONFIG_HOME to our temp directory
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Load cache configuration
    let cache_config = CacheConfigLoader::load().unwrap();

    // Verify config file was applied
    assert!(!cache_config.global.enabled);
    assert_eq!(cache_config.global.mode, cuenv::cache::CacheMode::Read);
    assert_eq!(cache_config.global.max_size, Some(5242880));
    assert_eq!(cache_config.global.inline_threshold, Some(2048));

    // Clean up
    std::env::remove_var("XDG_CONFIG_HOME");
}

#[tokio::test]
async fn test_cache_config_precedence_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("cuenv");
    std::fs::create_dir_all(&config_dir).unwrap();

    // Create config file with read-only mode
    let config_file = config_dir.join("config.json");
    let config_content = r#"
    {
        "cache": {
            "enabled": true,
            "mode": "read"
        }
    }
    "#;

    std::fs::write(&config_file, config_content).unwrap();

    // Set XDG_CONFIG_HOME to our temp directory
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Set environment variable to override config file
    std::env::set_var("CUENV_CACHE", "read-write");

    // Load cache configuration
    let cache_config = CacheConfigLoader::load().unwrap();

    // Verify environment variable overrides config file
    assert!(cache_config.global.enabled);
    assert_eq!(cache_config.global.mode, cuenv::cache::CacheMode::ReadWrite);

    // Clean up
    std::env::remove_var("XDG_CONFIG_HOME");
    std::env::remove_var("CUENV_CACHE");
}
