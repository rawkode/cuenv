//! Common test utilities and helpers
//!
//! This module provides shared utilities for testing across the cuenv codebase,
//! reducing code duplication and improving test reliability.

pub mod cleanup;
pub mod test_isolation;

use cuenv::cache::{CacheConfig, ProductionCache};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Builder pattern for creating test cache instances with custom configurations
pub struct TestCacheBuilder {
    config: CacheConfig,
    temp_dir: Option<TempDir>,
}

impl TestCacheBuilder {
    /// Create a new test cache builder with default configuration
    pub fn new() -> Self {
        Self {
            config: CacheConfig::default(),
            temp_dir: None,
        }
    }

    /// Set memory limit for the cache
    pub fn with_memory_limit(mut self, limit_bytes: u64) -> Self {
        self.config.max_size_bytes = limit_bytes;
        self
    }

    /// Set maximum number of cache entries
    pub fn with_max_entries(mut self, max_entries: u64) -> Self {
        self.config.max_entries = max_entries;
        self
    }

    /// Enable compression for the cache
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.config.compression_enabled = enabled;
        self
    }

    /// Build the cache instance, returning both cache and temp directory
    /// The temp directory must be kept alive for the cache to function
    pub async fn build(
        self,
    ) -> Result<(ProductionCache, TempDir), Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), self.config).await?;
        Ok((cache, temp_dir))
    }

    /// Build the cache with a specific directory path
    pub async fn build_at_path(
        self,
        path: PathBuf,
    ) -> Result<ProductionCache, Box<dyn std::error::Error + Send + Sync>> {
        let cache = ProductionCache::new(path, self.config).await?;
        Ok(cache)
    }
}

impl Default for TestCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a test environment with a CUE configuration file
pub fn setup_test_env_with_cue(cue_content: &str) -> Result<TempDir, std::io::Error> {
    let temp_dir = TempDir::new()?;
    fs::write(temp_dir.path().join("env.cue"), cue_content)?;
    Ok(temp_dir)
}

/// Create a test directory structure with multiple files
pub fn setup_test_project_structure() -> Result<TempDir, std::io::Error> {
    let temp_dir = TempDir::new()?;

    // Create basic project structure
    fs::create_dir_all(temp_dir.path().join("src"))?;
    fs::create_dir_all(temp_dir.path().join("tests"))?;
    fs::create_dir_all(temp_dir.path().join("build"))?;

    // Create some test files
    fs::write(
        temp_dir.path().join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;
    fs::write(
        temp_dir.path().join("tests/test.rs"),
        "#[test] fn test_example() { assert!(true); }",
    )?;
    fs::write(temp_dir.path().join("README.md"), "# Test Project")?;

    // Create env.cue with basic configuration
    let cue_content = r#"package cuenv

env: {
    PROJECT_NAME: "test_project"
    DEBUG: "true"
}

tasks: {
    build: {
        command: "cargo build"
        inputs: ["src/**/*.rs"]
        outputs: ["target/debug/test_project"]
    }
    test: {
        command: "cargo test"
        inputs: ["src/**/*.rs", "tests/**/*.rs"]
    }
}
"#;
    fs::write(temp_dir.path().join("env.cue"), cue_content)?;

    Ok(temp_dir)
}

/// Helper to create a CUE file with environment variables
pub fn create_env_cue_with_vars(vars: &[(&str, &str)]) -> String {
    let mut content = String::from("package cuenv\n\nenv: {\n");

    for (key, value) in vars {
        content.push_str(&format!("    {}: \"{}\"\n", key, value));
    }

    content.push_str("}\n");
    content
}

/// Helper to create a CUE file with tasks
pub fn create_env_cue_with_tasks(tasks: &[(&str, &str)]) -> String {
    let mut content = String::from("package cuenv\n\ntasks: {\n");

    for (name, command) in tasks {
        content.push_str(&format!(
            "    {}: {{\n        command: \"{}\"\n    }}\n",
            name, command
        ));
    }

    content.push_str("}\n");
    content
}

/// Error handling helper that provides better error messages in tests
pub fn expect_ok<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("Expected Ok but got Err in {}: {}", context, e),
    }
}

/// Error handling helper for async operations
pub async fn expect_ok_async<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("Expected Ok but got Err in {}: {}", context, e),
    }
}

/// Helper to assert that a result is an error with a specific message pattern
pub fn assert_error_contains<T, E: std::fmt::Display>(
    result: Result<T, E>,
    expected_message: &str,
    context: &str,
) {
    match result {
        Ok(_) => panic!(
            "Expected error containing '{}' but got Ok in {}",
            expected_message, context
        ),
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains(expected_message),
                "Expected error message to contain '{}' but got '{}' in {}",
                expected_message,
                error_msg,
                context
            );
        }
    }
}

/// Mock filesystem setup for testing file operations
pub struct MockFileSystem {
    pub temp_dir: TempDir,
}

impl MockFileSystem {
    /// Create a new mock filesystem
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self {
            temp_dir: TempDir::new()?,
        })
    }

    /// Create a file with given content
    pub fn create_file(&self, path: &str, content: &str) -> Result<PathBuf, std::io::Error> {
        let file_path = self.temp_dir.path().join(path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&file_path, content)?;
        Ok(file_path)
    }

    /// Create a directory
    pub fn create_dir(&self, path: &str) -> Result<PathBuf, std::io::Error> {
        let dir_path = self.temp_dir.path().join(path);
        fs::create_dir_all(&dir_path)?;
        Ok(dir_path)
    }

    /// Get the root path of the mock filesystem
    pub fn root_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new().expect("Failed to create mock filesystem")
    }
}

/// Test configuration constants
pub mod test_constants {
    use std::time::Duration;

    /// Standard timeout for async operations in tests
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Shorter timeout for operations that should be fast
    pub const FAST_TIMEOUT: Duration = Duration::from_secs(5);

    /// Timeout for operations that might be slow (e.g., network)
    pub const SLOW_TIMEOUT: Duration = Duration::from_secs(60);

    /// Small test data size
    pub const SMALL_DATA_SIZE: usize = 1024;

    /// Medium test data size
    pub const MEDIUM_DATA_SIZE: usize = 64 * 1024;

    /// Large test data size (for stress testing)
    pub const LARGE_DATA_SIZE: usize = 1024 * 1024;
}

/// Retry helper for flaky tests
pub async fn retry_async<F, T, E>(
    mut operation: F,
    max_attempts: u32,
    delay: std::time::Duration,
) -> Result<T, E>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_attempts {
                    last_error = Some(e);
                    break;
                } else {
                    println!("Attempt {} failed, retrying in {:?}: {}", attempt, delay, e);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_builder() {
        let (cache, _temp_dir) = TestCacheBuilder::new()
            .with_memory_limit(1024 * 1024)
            .with_max_entries(100)
            .build()
            .await
            .expect("Failed to build test cache");

        // Test that cache works
        cache
            .put("test_key", "test_value", None)
            .await
            .expect("Put should work");
        let value: Option<String> = cache.get("test_key").await.expect("Get should work");
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_setup_test_env_with_cue() {
        let cue_content = "package cuenv\nenv: { TEST: \"value\" }";
        let temp_dir = setup_test_env_with_cue(cue_content).expect("Should create test env");

        let env_file = temp_dir.path().join("env.cue");
        assert!(env_file.exists());

        let content = fs::read_to_string(env_file).expect("Should read file");
        assert!(content.contains("TEST"));
    }

    #[test]
    fn test_mock_filesystem() {
        let mock_fs = MockFileSystem::new().expect("Should create mock fs");

        mock_fs
            .create_file("test.txt", "hello world")
            .expect("Should create file");
        mock_fs
            .create_dir("subdir")
            .expect("Should create directory");

        let file_path = mock_fs.root_path().join("test.txt");
        assert!(file_path.exists());

        let content = fs::read_to_string(file_path).expect("Should read file");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_error_helpers() {
        // Test expect_ok
        let ok_result: Result<i32, &str> = Ok(42);
        let value = expect_ok(ok_result, "test context");
        assert_eq!(value, 42);

        // Test assert_error_contains
        let err_result: Result<i32, &str> = Err("file not found");
        assert_error_contains(err_result, "not found", "test context");
    }

    #[test]
    fn test_cue_generators() {
        let vars = [("KEY1", "value1"), ("KEY2", "value2")];
        let cue_content = create_env_cue_with_vars(&vars);

        assert!(cue_content.contains("KEY1: \"value1\""));
        assert!(cue_content.contains("KEY2: \"value2\""));
        assert!(cue_content.contains("package cuenv"));

        let tasks = [("build", "cargo build"), ("test", "cargo test")];
        let task_cue = create_env_cue_with_tasks(&tasks);

        assert!(task_cue.contains("build: {"));
        assert!(task_cue.contains("command: \"cargo build\""));
    }
}
