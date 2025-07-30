//! Integration tests for cache key generation with real tasks
//!
//! This module tests the integration of cache key generation with the
//! actual task execution system, ensuring that cache keys work correctly
//! in real-world scenarios.

use cuenv::cache::{CacheKeyGenerator, CacheManager};
use cuenv::cue_parser::{CacheEnvConfig, TaskConfig};
use std::collections::HashMap;
use std::path::PathBuf;

/// Test cache key generation with real Cargo build task
#[test]
fn test_cargo_build_cache_key_generation() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for Cargo build
    let cargo_config = CacheEnvConfig {
        include: Some(vec![
            "CARGO_*".to_string(),
            "RUST*".to_string(),
            "PATH".to_string(),
            "HOME".to_string(),
            "CC".to_string(),
            "CXX".to_string(),
        ]),
        exclude: Some(vec![
            "PS1".to_string(),
            "TERM".to_string(),
            "PWD".to_string(),
        ]),
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("cargo-build", cargo_config.into())
        .unwrap();

    let task_config = TaskConfig {
        description: Some("Cargo build task".to_string()),
        command: Some("cargo build --release".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.rs".to_string(), "Cargo.toml".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Simulate real Cargo build environment
    let cargo_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/home/user/.cargo/bin".to_string(),
        ),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CARGO_HOME".to_string(), "/home/user/.cargo".to_string()),
        ("RUSTUP_HOME".to_string(), "/home/user/.rustup".to_string()),
        ("RUSTFLAGS".to_string(), "-C opt-level=3".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CXX".to_string(), "g++".to_string()),
        ("PS1".to_string(), "$ ".to_string()),
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("PWD".to_string(), "/project".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "cargo_build_config_hash";
    let input_files = HashMap::from([
        ("src/main.rs".to_string(), "abc123".to_string()),
        ("Cargo.toml".to_string(), "def456".to_string()),
    ]);
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "cargo-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &cargo_env,
            command,
        )
        .unwrap();

    // Same task with different irrelevant env vars should produce same key
    let mut cargo_env2 = cargo_env.clone();
    cargo_env2.insert("PS1".to_string(), "cargo> ".to_string());
    cargo_env2.insert("TERM".to_string(), "screen-256color".to_string());

    let key2 = generator
        .generate_cache_key(
            "cargo-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &cargo_env2,
            command,
        )
        .unwrap();

    assert_eq!(
        key1, key2,
        "Cache keys should be identical with only irrelevant env var changes"
    );

    // Different RUSTFLAGS should produce different key
    let mut cargo_env3 = cargo_env.clone();
    cargo_env3.insert("RUSTFLAGS".to_string(), "-C opt-level=0".to_string());

    let key3 = generator
        .generate_cache_key(
            "cargo-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &cargo_env3,
            command,
        )
        .unwrap();

    assert_ne!(
        key1, key3,
        "Cache keys should differ when RUSTFLAGS changes"
    );
}

/// Test cache key generation with npm build task
#[test]
fn test_npm_build_cache_key_generation() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for npm build
    let npm_config = CacheEnvConfig {
        include: Some(vec![
            "npm_config_*".to_string(),
            "NODE_*".to_string(),
            "NPM_*".to_string(),
            "PATH".to_string(),
            "HOME".to_string(),
        ]),
        exclude: Some(vec![
            "PS1".to_string(),
            "TERM".to_string(),
            "PWD".to_string(),
        ]),
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("npm-build", npm_config.into())
        .unwrap();

    let task_config = TaskConfig {
        description: Some("npm build task".to_string()),
        command: Some("npm run build".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.js".to_string(), "package.json".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Simulate real npm build environment
    let npm_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/usr/local/bin".to_string(),
        ),
        ("HOME".to_string(), "/home/user".to_string()),
        ("NODE_ENV".to_string(), "production".to_string()),
        ("npm_config_cache".to_string(), "/tmp/npm-cache".to_string()),
        ("npm_config_prefix".to_string(), "/usr/local".to_string()),
        (
            "NPM_CONFIG_REGISTRY".to_string(),
            "https://registry.npmjs.org/".to_string(),
        ),
        ("PS1".to_string(), "$ ".to_string()),
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("PWD".to_string(), "/project".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "npm_build_config_hash";
    let input_files = HashMap::from([
        ("src/index.js".to_string(), "abc123".to_string()),
        ("package.json".to_string(), "def456".to_string()),
    ]);
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "npm-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &npm_env,
            command,
        )
        .unwrap();

    // Same task with different NODE_ENV should produce different key
    let mut npm_env2 = npm_env.clone();
    npm_env2.insert("NODE_ENV".to_string(), "development".to_string());

    let key2 = generator
        .generate_cache_key(
            "npm-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &npm_env2,
            command,
        )
        .unwrap();

    assert_ne!(key1, key2, "Cache keys should differ when NODE_ENV changes");

    // Different npm config should produce different key
    let mut npm_env3 = npm_env.clone();
    npm_env3.insert(
        "npm_config_cache".to_string(),
        "/tmp/npm-cache-dev".to_string(),
    );

    let key3 = generator
        .generate_cache_key(
            "npm-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &npm_env3,
            command,
        )
        .unwrap();

    assert_ne!(
        key1, key3,
        "Cache keys should differ when npm_config_cache changes"
    );
}

/// Test cache key generation with Python build task
#[test]
fn test_python_build_cache_key_generation() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for Python build
    let python_config = CacheEnvConfig {
        include: Some(vec![
            "PYTHON*".to_string(),
            "PIP_*".to_string(),
            "VIRTUAL_ENV".to_string(),
            "CONDA_*".to_string(),
            "PATH".to_string(),
            "HOME".to_string(),
        ]),
        exclude: Some(vec![
            "PS1".to_string(),
            "TERM".to_string(),
            "PWD".to_string(),
        ]),
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("python-build", python_config.into())
        .unwrap();

    let task_config = TaskConfig {
        description: Some("Python build task".to_string()),
        command: Some("python setup.py build".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.py".to_string(), "setup.py".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Simulate real Python build environment
    let python_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/home/user/.pyenv/bin".to_string(),
        ),
        ("HOME".to_string(), "/home/user".to_string()),
        ("PYTHONPATH".to_string(), "/project/src".to_string()),
        ("VIRTUAL_ENV".to_string(), "/home/user/venv".to_string()),
        ("PIPENV_ACTIVE".to_string(), "1".to_string()),
        ("PIP_CACHE_DIR".to_string(), "/tmp/pip-cache".to_string()),
        ("PS1".to_string(), "(venv) $ ".to_string()),
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("PWD".to_string(), "/project".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "python_build_config_hash";
    let input_files = HashMap::from([
        ("src/main.py".to_string(), "abc123".to_string()),
        ("setup.py".to_string(), "def456".to_string()),
    ]);
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "python-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &python_env,
            command,
        )
        .unwrap();

    // Same task with different VIRTUAL_ENV should produce different key
    let mut python_env2 = python_env.clone();
    python_env2.insert("VIRTUAL_ENV".to_string(), "/home/user/venv2".to_string());

    let key2 = generator
        .generate_cache_key(
            "python-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &python_env2,
            command,
        )
        .unwrap();

    assert_ne!(
        key1, key2,
        "Cache keys should differ when VIRTUAL_ENV changes"
    );

    // Different PYTHONPATH should produce different key
    let mut python_env3 = python_env.clone();
    python_env3.insert(
        "PYTHONPATH".to_string(),
        "/project/src:/project/lib".to_string(),
    );

    let key3 = generator
        .generate_cache_key(
            "python-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &python_env3,
            command,
        )
        .unwrap();

    assert_ne!(
        key1, key3,
        "Cache keys should differ when PYTHONPATH changes"
    );
}

/// Test cache key generation with Make build task
#[test]
fn test_make_build_cache_key_generation() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for Make build
    let make_config = CacheEnvConfig {
        include: Some(vec![
            "CC".to_string(),
            "CXX".to_string(),
            "CFLAGS".to_string(),
            "CXXFLAGS".to_string(),
            "LDFLAGS".to_string(),
            "MAKEFLAGS".to_string(),
            "PATH".to_string(),
            "HOME".to_string(),
        ]),
        exclude: Some(vec![
            "PS1".to_string(),
            "TERM".to_string(),
            "PWD".to_string(),
        ]),
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("make-build", make_config.into())
        .unwrap();

    let task_config = TaskConfig {
        description: Some("Make build task".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string(), "Makefile".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Simulate real Make build environment
    let make_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CXX".to_string(), "g++".to_string()),
        ("CFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("CXXFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("LDFLAGS".to_string(), "-lm".to_string()),
        ("MAKEFLAGS".to_string(), "-j4".to_string()),
        ("PS1".to_string(), "$ ".to_string()),
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("PWD".to_string(), "/project".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "make_build_config_hash";
    let input_files = HashMap::from([
        ("src/main.c".to_string(), "abc123".to_string()),
        ("Makefile".to_string(), "def456".to_string()),
    ]);
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "make-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &make_env,
            command,
        )
        .unwrap();

    // Same task with different CFLAGS should produce different key
    let mut make_env2 = make_env.clone();
    make_env2.insert("CFLAGS".to_string(), "-O0 -g -Wall".to_string());

    let key2 = generator
        .generate_cache_key(
            "make-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &make_env2,
            command,
        )
        .unwrap();

    assert_ne!(key1, key2, "Cache keys should differ when CFLAGS changes");

    // Different MAKEFLAGS should produce different key
    let mut make_env3 = make_env.clone();
    make_env3.insert("MAKEFLAGS".to_string(), "-j8".to_string());

    let key3 = generator
        .generate_cache_key(
            "make-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &make_env3,
            command,
        )
        .unwrap();

    assert_ne!(
        key1, key3,
        "Cache keys should differ when MAKEFLAGS changes"
    );
}

/// Test cache key generation with mixed build tools
#[test]
fn test_mixed_build_tools_cache_key_generation() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for different build tools
    let cargo_config = CacheEnvConfig {
        include: Some(vec!["CARGO_*".to_string(), "RUST*".to_string()]),
        exclude: None,
        use_smart_defaults: Some(true),
    };

    let npm_config = CacheEnvConfig {
        include: Some(vec!["npm_config_*".to_string(), "NODE_*".to_string()]),
        exclude: None,
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("cargo-build", cargo_config.into())
        .unwrap();
    generator
        .add_task_config("npm-build", npm_config.into())
        .unwrap();

    let cargo_task = TaskConfig {
        description: Some("Cargo build task".to_string()),
        command: Some("cargo build".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.rs".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    let npm_task = TaskConfig {
        description: Some("npm build task".to_string()),
        command: Some("npm run build".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.js".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Mixed environment with both Cargo and npm variables
    let mixed_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CARGO_HOME".to_string(), "/home/user/.cargo".to_string()),
        ("RUSTFLAGS".to_string(), "-O2".to_string()),
        ("NODE_ENV".to_string(), "production".to_string()),
        ("npm_config_cache".to_string(), "/tmp/npm-cache".to_string()),
        ("PS1".to_string(), "$ ".to_string()),
        ("TERM".to_string(), "xterm".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "mixed_build_config_hash";
    let input_files = HashMap::from([("src/main.rs".to_string(), "abc123".to_string())]);
    let cargo_command = cargo_task.command.as_deref();
    let npm_command = npm_task.command.as_deref();

    let cargo_key = generator
        .generate_cache_key(
            "cargo-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &mixed_env,
            cargo_command,
        )
        .unwrap();

    let npm_key = generator
        .generate_cache_key(
            "npm-build",
            task_config_hash,
            &working_dir,
            &input_files,
            &mixed_env,
            npm_command,
        )
        .unwrap();

    // Keys should be different because different task configurations
    assert_ne!(
        cargo_key, npm_key,
        "Cache keys should differ for different build tools"
    );

    // Verify that cargo key includes Cargo variables but not npm variables
    let cargo_filtered = generator.filter_env_vars("cargo-build", &mixed_env);
    assert!(cargo_filtered.contains_key("CARGO_HOME"));
    assert!(cargo_filtered.contains_key("RUSTFLAGS"));
    assert!(!cargo_filtered.contains_key("NODE_ENV"));
    assert!(!cargo_filtered.contains_key("npm_config_cache"));

    // Verify that npm key includes npm variables but not Cargo variables
    let npm_filtered = generator.filter_env_vars("npm-build", &mixed_env);
    assert!(!npm_filtered.contains_key("CARGO_HOME"));
    assert!(!npm_filtered.contains_key("RUSTFLAGS"));
    assert!(npm_filtered.contains_key("NODE_ENV"));
    assert!(npm_filtered.contains_key("npm_config_cache"));
}

/// Test integration with CacheManager
#[test]
fn test_cache_manager_integration() {
    // Create a temporary directory for cache
    let _temp_dir = tempfile::tempdir().unwrap(); // Prefix with underscore to indicate intentional unused

    // Create cache manager
    let cache_manager = CacheManager::new_sync().unwrap();

    // Test task configuration
    let task_config = TaskConfig {
        description: Some("Integration test task".to_string()),
        command: Some("echo test".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: None,
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Test environment variables
    let env_vars = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("PS1".to_string(), "$ ".to_string()),
        ("TERM".to_string(), "xterm".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");

    // Generate cache key using cache manager
    let cache_key = cache_manager
        .generate_cache_key("test-task", &task_config, &env_vars, &working_dir)
        .unwrap();

    // Verify cache key is not empty
    assert!(!cache_key.is_empty());

    // Generate cache key again with same inputs
    let cache_key2 = cache_manager
        .generate_cache_key("test-task", &task_config, &env_vars, &working_dir)
        .unwrap();

    // Keys should be identical
    assert_eq!(
        cache_key, cache_key2,
        "Cache keys should be identical for same inputs"
    );

    // Generate cache key with different irrelevant env vars
    let mut env_vars2 = env_vars.clone();
    env_vars2.insert("PS1".to_string(), "test> ".to_string());

    let cache_key3 = cache_manager
        .generate_cache_key("test-task", &task_config, &env_vars2, &working_dir)
        .unwrap();

    // Keys should still be identical (only irrelevant env vars changed)
    assert_eq!(
        cache_key, cache_key3,
        "Cache keys should be identical with only irrelevant env var changes"
    );

    // Generate cache key with different relevant env vars
    let mut env_vars3 = env_vars.clone();
    env_vars3.insert(
        "PATH".to_string(),
        "/usr/local/bin:/usr/bin:/bin".to_string(),
    );

    let cache_key4 = cache_manager
        .generate_cache_key("test-task", &task_config, &env_vars3, &working_dir)
        .unwrap();

    // Keys should be different (relevant env var changed)
    assert_ne!(
        cache_key, cache_key4,
        "Cache keys should differ when relevant env vars change"
    );
}
