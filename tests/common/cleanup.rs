//! Test cleanup utilities to ensure proper resource management
//!
//! This module provides patterns and utilities for cleaning up test resources,
//! preventing test interference and resource leaks.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Global cleanup registry to track resources that need cleanup
static CLEANUP_REGISTRY: OnceLock<Arc<Mutex<CleanupRegistry>>> = OnceLock::new();

/// Registry of cleanup tasks that need to be executed
struct CleanupRegistry {
    cleanup_tasks: Vec<Box<dyn FnOnce() + Send>>,
    temp_directories: Vec<PathBuf>,
    environment_vars: HashMap<String, Option<String>>, // Original values
}

impl CleanupRegistry {
    fn new() -> Self {
        Self {
            cleanup_tasks: Vec::new(),
            temp_directories: Vec::new(),
            environment_vars: HashMap::new(),
        }
    }

    fn register_cleanup<F>(&mut self, cleanup_fn: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.cleanup_tasks.push(Box::new(cleanup_fn));
    }

    fn register_temp_dir(&mut self, path: PathBuf) {
        self.temp_directories.push(path);
    }

    fn register_env_var(&mut self, key: String, original_value: Option<String>) {
        self.environment_vars.insert(key, original_value);
    }

    fn execute_all_cleanup(&mut self) {
        // Execute custom cleanup tasks
        for cleanup_task in self.cleanup_tasks.drain(..) {
            cleanup_task();
        }

        // Clean up temporary directories
        for temp_dir in self.temp_directories.drain(..) {
            if temp_dir.exists() {
                if let Err(e) = fs::remove_dir_all(&temp_dir) {
                    eprintln!("Warning: Failed to clean up temp dir {:?}: {}", temp_dir, e);
                }
            }
        }

        // Restore environment variables
        for (key, original_value) in self.environment_vars.drain() {
            match original_value {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }
}

/// Get the global cleanup registry
fn get_cleanup_registry() -> Arc<Mutex<CleanupRegistry>> {
    CLEANUP_REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(CleanupRegistry::new())))
        .clone()
}

/// RAII guard that ensures cleanup happens when dropped
pub struct CleanupGuard {
    registry: Arc<Mutex<CleanupRegistry>>,
}

impl CleanupGuard {
    /// Create a new cleanup guard
    pub fn new() -> Self {
        Self {
            registry: get_cleanup_registry(),
        }
    }

    /// Register a custom cleanup function
    pub fn register_cleanup<F>(&self, cleanup_fn: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut registry = self.registry.lock().unwrap();
        registry.register_cleanup(cleanup_fn);
    }

    /// Register a temporary directory for cleanup
    pub fn register_temp_dir<P: AsRef<Path>>(&self, path: P) {
        let mut registry = self.registry.lock().unwrap();
        registry.register_temp_dir(path.as_ref().to_path_buf());
    }

    /// Set an environment variable and register it for cleanup
    pub fn set_env_var(&self, key: &str, value: &str) {
        let original = std::env::var(key).ok();
        std::env::set_var(key, value);

        let mut registry = self.registry.lock().unwrap();
        registry.register_env_var(key.to_string(), original);
    }

    /// Remove an environment variable and register it for restoration
    pub fn remove_env_var(&self, key: &str) {
        let original = std::env::var(key).ok();
        std::env::remove_var(key);

        let mut registry = self.registry.lock().unwrap();
        registry.register_env_var(key.to_string(), original);
    }

    /// Execute all cleanup tasks immediately
    pub fn cleanup_now(&self) {
        let mut registry = self.registry.lock().unwrap();
        registry.execute_all_cleanup();
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let mut registry = self.registry.lock().unwrap();
        registry.execute_all_cleanup();
    }
}

impl Default for CleanupGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Scoped environment variable manager
pub struct ScopedEnv {
    cleanup_guard: CleanupGuard,
}

impl ScopedEnv {
    /// Create a new scoped environment manager
    pub fn new() -> Self {
        Self {
            cleanup_guard: CleanupGuard::new(),
        }
    }

    /// Set an environment variable for the duration of this scope
    pub fn set(&self, key: &str, value: &str) -> &Self {
        self.cleanup_guard.set_env_var(key, value);
        self
    }

    /// Remove an environment variable for the duration of this scope
    pub fn remove(&self, key: &str) -> &Self {
        self.cleanup_guard.remove_env_var(key);
        self
    }

    /// Set multiple environment variables
    pub fn set_many<I, K, V>(&self, vars: I) -> &Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for (key, value) in vars {
            self.set(key.as_ref(), value.as_ref());
        }
        self
    }
}

impl Default for ScopedEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// File system cleanup utilities
pub struct FileSystemCleanup {
    cleanup_guard: CleanupGuard,
    managed_paths: Vec<PathBuf>,
}

impl FileSystemCleanup {
    /// Create a new file system cleanup manager
    pub fn new() -> Self {
        Self {
            cleanup_guard: CleanupGuard::new(),
            managed_paths: Vec::new(),
        }
    }

    /// Create a temporary file and register it for cleanup
    pub fn create_temp_file(&mut self, content: &str) -> Result<PathBuf, std::io::Error> {
        let temp_file = tempfile::NamedTempFile::new()?;
        let path = temp_file.path().to_path_buf();

        fs::write(&path, content)?;

        // Keep the temp file alive and register for cleanup
        self.cleanup_guard.register_cleanup(move || {
            let _ = fs::remove_file(&path);
        });

        // Prevent automatic deletion by taking ownership
        let (_, persistent_path) = temp_file.keep()?;
        self.managed_paths.push(persistent_path.clone());

        Ok(persistent_path)
    }

    /// Create a temporary directory and register it for cleanup
    pub fn create_temp_dir(&mut self) -> Result<PathBuf, std::io::Error> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().to_path_buf();

        self.cleanup_guard.register_temp_dir(path.clone());

        // Prevent automatic deletion
        let persistent_path = temp_dir.into_path();
        self.managed_paths.push(persistent_path.clone());

        Ok(persistent_path)
    }

    /// Create a test file at a specific path and register it for cleanup
    pub fn create_test_file<P: AsRef<Path>>(
        &mut self,
        path: P,
        content: &str,
    ) -> Result<PathBuf, std::io::Error> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;

        let path_clone = path.clone();
        self.cleanup_guard.register_cleanup(move || {
            let _ = fs::remove_file(&path_clone);
        });

        self.managed_paths.push(path.clone());
        Ok(path)
    }

    /// Get all managed paths
    pub fn managed_paths(&self) -> &[PathBuf] {
        &self.managed_paths
    }
}

impl Default for FileSystemCleanup {
    fn default() -> Self {
        Self::new()
    }
}

/// Test timeout helper with cleanup
pub struct TestTimeout {
    start_time: Instant,
    timeout: Duration,
    cleanup_guard: CleanupGuard,
}

impl TestTimeout {
    /// Create a new test timeout
    pub fn new(timeout: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            timeout,
            cleanup_guard: CleanupGuard::new(),
        }
    }

    /// Check if the test has timed out
    pub fn is_expired(&self) -> bool {
        self.start_time.elapsed() > self.timeout
    }

    /// Get remaining time
    pub fn remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.start_time.elapsed())
    }

    /// Assert that the test hasn't timed out
    pub fn assert_not_expired(&self, context: &str) {
        if self.is_expired() {
            panic!(
                "Test timed out after {:?} in context: {}",
                self.timeout, context
            );
        }
    }

    /// Register a cleanup function to run if the test times out
    pub fn register_cleanup<F>(&self, cleanup_fn: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.cleanup_guard.register_cleanup(cleanup_fn);
    }

    /// Get the underlying cleanup guard
    pub fn cleanup_guard(&self) -> &CleanupGuard {
        &self.cleanup_guard
    }
}

/// Macro to create a test with automatic cleanup
#[macro_export]
macro_rules! test_with_cleanup {
    ($test_name:ident, $test_body:expr) => {
        #[test]
        fn $test_name() {
            let _cleanup_guard = $crate::common::cleanup::CleanupGuard::new();
            $test_body(_cleanup_guard);
        }
    };
}

/// Macro to create an async test with automatic cleanup
#[macro_export]
macro_rules! async_test_with_cleanup {
    ($test_name:ident, $test_body:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let _cleanup_guard = $crate::common::cleanup::CleanupGuard::new();
            $test_body(_cleanup_guard).await;
        }
    };
}

/// Cleanup helper for process-related resources
pub struct ProcessCleanup {
    cleanup_guard: CleanupGuard,
    child_processes: Vec<u32>, // PIDs
}

impl ProcessCleanup {
    /// Create a new process cleanup manager
    pub fn new() -> Self {
        Self {
            cleanup_guard: CleanupGuard::new(),
            child_processes: Vec::new(),
        }
    }

    /// Register a child process for cleanup
    pub fn register_process(&mut self, pid: u32) {
        self.child_processes.push(pid);

        self.cleanup_guard.register_cleanup(move || {
            // Attempt to terminate the process
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }

            #[cfg(windows)]
            {
                // Windows process termination would go here
                // For now, just log
                eprintln!("Warning: Process cleanup not implemented for Windows");
            }
        });
    }

    /// Kill all registered processes immediately
    pub fn kill_all_processes(&self) {
        for &pid in &self.child_processes {
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }
    }
}

impl Default for ProcessCleanup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_cleanup_guard_env_vars() {
        let original_value = env::var("TEST_CLEANUP_VAR").ok();

        {
            let cleanup = CleanupGuard::new();
            cleanup.set_env_var("TEST_CLEANUP_VAR", "test_value");

            assert_eq!(env::var("TEST_CLEANUP_VAR").unwrap(), "test_value");
        } // cleanup drops here

        // Should be restored to original value
        assert_eq!(env::var("TEST_CLEANUP_VAR").ok(), original_value);
    }

    #[test]
    fn test_scoped_env() {
        let original_value = env::var("TEST_SCOPED_VAR").ok();

        {
            let scoped_env = ScopedEnv::new();
            scoped_env.set("TEST_SCOPED_VAR", "scoped_value");

            assert_eq!(env::var("TEST_SCOPED_VAR").unwrap(), "scoped_value");
        } // scoped_env drops here

        assert_eq!(env::var("TEST_SCOPED_VAR").ok(), original_value);
    }

    #[test]
    fn test_filesystem_cleanup() {
        let mut fs_cleanup = FileSystemCleanup::new();

        let temp_file = fs_cleanup
            .create_temp_file("test content")
            .expect("Should create temp file");

        assert!(temp_file.exists());
        assert_eq!(fs::read_to_string(&temp_file).unwrap(), "test content");

        // File should exist during the test
        assert!(!fs_cleanup.managed_paths().is_empty());

        // Manual cleanup for testing
        fs_cleanup.cleanup_guard.cleanup_now();
    }

    #[test]
    fn test_test_timeout() {
        let timeout = TestTimeout::new(Duration::from_millis(100));

        assert!(!timeout.is_expired());
        assert!(timeout.remaining() > Duration::from_millis(50));

        std::thread::sleep(Duration::from_millis(150));

        assert!(timeout.is_expired());
        assert_eq!(timeout.remaining(), Duration::ZERO);
    }

    #[test]
    #[should_panic(expected = "Test timed out")]
    fn test_timeout_assertion() {
        let timeout = TestTimeout::new(Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(100));
        timeout.assert_not_expired("test context");
    }

    #[test]
    fn test_multiple_cleanup_registrations() {
        let cleanup = CleanupGuard::new();
        let counter = Arc::new(Mutex::new(0));

        let counter_clone = Arc::clone(&counter);
        cleanup.register_cleanup(move || {
            *counter_clone.lock().unwrap() += 1;
        });

        let counter_clone = Arc::clone(&counter);
        cleanup.register_cleanup(move || {
            *counter_clone.lock().unwrap() += 10;
        });

        cleanup.cleanup_now();

        // Both cleanup functions should have executed
        assert_eq!(*counter.lock().unwrap(), 11);
    }
}
