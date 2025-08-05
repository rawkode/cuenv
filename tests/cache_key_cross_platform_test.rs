//! Cross-platform compatibility tests for cache key generation
//!
//! This module tests that cache key generation works consistently across
//! different platforms (Linux, macOS, Windows) and handles platform-specific
//! environment variables appropriately.

use cuenv::cache::CacheKeyGenerator;
use cuenv::config::{CacheEnvConfig, TaskConfig};
use std::collections::HashMap;
use std::path::PathBuf;

/// Test cache key generation with platform-specific environment variables
#[test]
fn test_platform_specific_env_vars() {
    let generator = CacheKeyGenerator::new().unwrap();

    let task_config = TaskConfig {
        description: Some("Cross-platform build".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Linux-specific environment variables
    let linux_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("USER".to_string(), "user".to_string()),
        ("SHELL".to_string(), "/bin/bash".to_string()),
        ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
        (
            "XDG_DATA_DIRS".to_string(),
            "/usr/local/share:/usr/share".to_string(),
        ),
        ("DISPLAY".to_string(), ":0".to_string()),
        (
            "DBUS_SESSION_BUS_ADDRESS".to_string(),
            "unix:path=/run/user/1000/bus".to_string(),
        ),
        ("CC".to_string(), "gcc".to_string()),
        ("CFLAGS".to_string(), "-O2".to_string()),
    ]);

    // macOS-specific environment variables
    let macos_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/usr/local/bin".to_string(),
        ),
        ("HOME".to_string(), "/Users/user".to_string()),
        ("USER".to_string(), "user".to_string()),
        ("SHELL".to_string(), "/bin/zsh".to_string()),
        (
            "DISPLAY".to_string(),
            "/private/tmp/com.apple.launchd.12345/org.macosforge.xquartz:0".to_string(),
        ),
        ("SECURITYSESSIONID".to_string(), "12345".to_string()),
        ("COMMAND_MODE".to_string(), "unix2003".to_string()),
        (
            "__CF_USER_TEXT_ENCODING".to_string(),
            "0x1F5:0x0:0x0".to_string(),
        ),
        ("CC".to_string(), "clang".to_string()),
        ("CFLAGS".to_string(), "-O2".to_string()),
    ]);

    // Windows-specific environment variables (WSL/Cygwin)
    let windows_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/mnt/c/Windows/System32".to_string(),
        ),
        ("HOME".to_string(), "/mnt/c/Users/user".to_string()),
        ("USER".to_string(), "user".to_string()),
        ("SHELL".to_string(), "/bin/bash".to_string()),
        ("WSL_DISTRO_NAME".to_string(), "Ubuntu".to_string()),
        (
            "WSL_INTEROP".to_string(),
            "/run/WSL/12345_interop".to_string(),
        ),
        ("COMPUTERNAME".to_string(), "DESKTOP-ABC123".to_string()),
        ("USERNAME".to_string(), "user".to_string()),
        ("USERDOMAIN".to_string(), "DESKTOP-ABC123".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CFLAGS".to_string(), "-O2".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "cross_platform_hash";
    let input_files = HashMap::new();
    let command = task_config.command.as_deref();

    let linux_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &linux_env,
            command,
        )
        .unwrap();
    let macos_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &macos_env,
            command,
        )
        .unwrap();
    let windows_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &windows_env,
            command,
        )
        .unwrap();

    // Keys should be identical since only platform-specific variables differ
    assert_eq!(
        linux_key, macos_key,
        "Cache keys should be identical across Linux and macOS"
    );
    assert_eq!(
        macos_key, windows_key,
        "Cache keys should be identical across macOS and Windows"
    );
    assert_eq!(
        linux_key, windows_key,
        "Cache keys should be identical across Linux and Windows"
    );
}

/// Test cache key generation with different path separators
#[test]
fn test_path_separator_normalization() {
    let generator = CacheKeyGenerator::new().unwrap();

    let task_config = TaskConfig {
        description: Some("Path normalization test".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    let env_vars = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
    ]);

    // Test with different path separators (should be normalized)
    let working_dir1 = PathBuf::from("/project");
    let working_dir2 = PathBuf::from("/project/");
    let working_dir3 = PathBuf::from("/project/.");
    let working_dir4 = PathBuf::from("/tmp/../project");

    let task_config_hash = "path_normalization_hash";
    let input_files = HashMap::new();
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir1,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key2 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir2,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key3 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir3,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key4 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir4,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();

    // All keys should be identical for equivalent paths
    assert_eq!(
        key1, key2,
        "Cache keys should be identical for /project and /project/"
    );
    assert_eq!(
        key2, key3,
        "Cache keys should be identical for /project/ and /project/."
    );
    assert_eq!(
        key3, key4,
        "Cache keys should be identical for /project/. and /tmp/../project"
    );
}

/// Test cache key generation with case sensitivity
#[test]
fn test_case_sensitivity_handling() {
    let generator = CacheKeyGenerator::new().unwrap();

    let task_config = TaskConfig {
        description: Some("Case sensitivity test".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Test with case-sensitive environment variables
    let env_vars1 = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CFLAGS".to_string(), "-O2".to_string()),
    ]);

    // Test with different case (should produce different keys on case-sensitive systems)
    let env_vars2 = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CC".to_string(), "GCC".to_string()), // Different case
        ("CFLAGS".to_string(), "-O2".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "case_sensitivity_hash";
    let input_files = HashMap::new();
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &env_vars1,
            command,
        )
        .unwrap();
    let key2 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &env_vars2,
            command,
        )
        .unwrap();

    // Keys should be different when case differs
    assert_ne!(
        key1, key2,
        "Cache keys should differ when environment variable case differs"
    );
}

/// Test cache key generation with Windows-style paths
#[test]
fn test_windows_path_handling() {
    let generator = CacheKeyGenerator::new().unwrap();

    let task_config = TaskConfig {
        description: Some("Windows path test".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    let env_vars = HashMap::from([
        (
            "PATH".to_string(),
            "C:\\Windows\\System32;C:\\Program Files\\Git\\bin".to_string(),
        ),
        ("HOME".to_string(), "C:\\Users\\user".to_string()),
        ("USERPROFILE".to_string(), "C:\\Users\\user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
    ]);

    // Test with Windows-style paths
    let working_dir1 = PathBuf::from("C:\\project");
    let working_dir2 = PathBuf::from("C:/project");
    let working_dir3 = PathBuf::from("C:\\project\\");
    let working_dir4 = PathBuf::from("C:/project/");

    let task_config_hash = "windows_path_hash";
    let input_files = HashMap::new();
    let command = task_config.command.as_deref();

    let key1 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir1,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key2 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir2,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key3 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir3,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();
    let key4 = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir4,
            &input_files,
            &env_vars,
            command,
        )
        .unwrap();

    // All keys should be identical for equivalent Windows paths
    assert_eq!(
        key1, key2,
        "Cache keys should be identical for C:\\project and C:/project"
    );
    assert_eq!(
        key2, key3,
        "Cache keys should be identical for C:/project and C:\\project\\"
    );
    assert_eq!(
        key3, key4,
        "Cache keys should be identical for C:\\project\\ and C:/project/"
    );
}

/// Test cache key generation with cross-platform build tools
#[test]
fn test_cross_platform_build_tools() {
    let mut generator = CacheKeyGenerator::new().unwrap();

    // Configure for cross-platform build
    let build_config = CacheEnvConfig {
        include: Some(vec![
            "CC".to_string(),
            "CXX".to_string(),
            "CFLAGS".to_string(),
            "CXXFLAGS".to_string(),
        ]),
        exclude: Some(vec!["TMP".to_string(), "TEMP".to_string()]),
        use_smart_defaults: Some(true),
    };

    generator
        .add_task_config("build", build_config.into())
        .unwrap();

    let task_config = TaskConfig {
        description: Some("Cross-platform build".to_string()),
        command: Some("make".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: Some(vec!["src/**/*.c".to_string()]),
        outputs: None,
        security: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
    };

    // Linux build environment
    let linux_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CXX".to_string(), "g++".to_string()),
        ("CFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("CXXFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("TMPDIR".to_string(), "/tmp".to_string()),
        ("TERM".to_string(), "xterm".to_string()),
    ]);

    // macOS build environment
    let macos_env = HashMap::from([
        (
            "PATH".to_string(),
            "/usr/bin:/bin:/usr/local/bin".to_string(),
        ),
        ("HOME".to_string(), "/Users/user".to_string()),
        ("CC".to_string(), "clang".to_string()),
        ("CXX".to_string(), "clang++".to_string()),
        ("CFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("CXXFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("TMPDIR".to_string(), "/tmp".to_string()),
        ("TERM".to_string(), "xterm-256color".to_string()),
    ]);

    // Windows build environment
    let windows_env = HashMap::from([
        (
            "PATH".to_string(),
            "C:\\Windows\\System32;C:\\Program Files\\Git\\bin".to_string(),
        ),
        ("HOME".to_string(), "C:\\Users\\user".to_string()),
        ("CC".to_string(), "gcc".to_string()),
        ("CXX".to_string(), "g++".to_string()),
        ("CFLAGS".to_string(), "-O2 -Wall".to_string()),
        ("CXXFLAGS".to_string(), "-O2 -Wall".to_string()),
        (
            "TMP".to_string(),
            "C:\\Users\\user\\AppData\\Local\\Temp".to_string(),
        ),
        (
            "TEMP".to_string(),
            "C:\\Users\\user\\AppData\\Local\\Temp".to_string(),
        ),
        ("TERM".to_string(), "xterm".to_string()),
    ]);

    let working_dir = PathBuf::from("/project");
    let task_config_hash = "cross_platform_build_hash";
    let input_files = HashMap::new();
    let command = task_config.command.as_deref();

    let linux_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &linux_env,
            command,
        )
        .unwrap();
    let macos_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &macos_env,
            command,
        )
        .unwrap();
    let windows_key = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &windows_env,
            command,
        )
        .unwrap();

    // Keys should be different because compilers differ
    assert_ne!(
        linux_key, macos_key,
        "Cache keys should differ between Linux (gcc) and macOS (clang)"
    );
    assert_ne!(
        macos_key, windows_key,
        "Cache keys should differ between macOS (clang) and Windows (gcc)"
    );
    assert_ne!(
        linux_key, windows_key,
        "Cache keys should differ between Linux (gcc) and Windows (gcc)"
    );

    // But if we use the same compiler, keys should be the same
    let mut macos_env_same_compiler = macos_env.clone();
    macos_env_same_compiler.insert("CC".to_string(), "gcc".to_string());
    macos_env_same_compiler.insert("CXX".to_string(), "g++".to_string());

    let macos_key_same_compiler = generator
        .generate_cache_key(
            "build",
            task_config_hash,
            &working_dir,
            &input_files,
            &macos_env_same_compiler,
            command,
        )
        .unwrap();

    assert_eq!(
        linux_key, macos_key_same_compiler,
        "Cache keys should be identical when using the same compiler"
    );
}
