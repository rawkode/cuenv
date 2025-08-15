//! Test isolation helpers to prevent resource exhaustion and conflicts
//!
//! This module provides utilities for running tests in isolated environments
//! with proper resource management and cleanup.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::runtime::{Builder, Runtime};

/// Global test resource manager to prevent resource exhaustion
static TEST_RESOURCE_MANAGER: OnceLock<Arc<Mutex<TestResourceManager>>> = OnceLock::new();

/// Manages test resources to prevent exhaustion
struct TestResourceManager {
    active_runtimes: usize,
    max_concurrent_runtimes: usize,
    active_heavy_tests: usize,
    max_concurrent_heavy_tests: usize,
}

impl TestResourceManager {
    fn new() -> Self {
        Self {
            active_runtimes: 0,
            max_concurrent_runtimes: 4, // Prevent TLS exhaustion
            active_heavy_tests: 0,
            max_concurrent_heavy_tests: 2, // Limit memory-intensive tests
        }
    }

    fn can_create_runtime(&self) -> bool {
        self.active_runtimes < self.max_concurrent_runtimes
    }

    fn can_run_heavy_test(&self) -> bool {
        self.active_heavy_tests < self.max_concurrent_heavy_tests
    }

    fn acquire_runtime(&mut self) -> bool {
        if self.can_create_runtime() {
            self.active_runtimes += 1;
            true
        } else {
            false
        }
    }

    fn release_runtime(&mut self) {
        if self.active_runtimes > 0 {
            self.active_runtimes -= 1;
        }
    }

    fn acquire_heavy_test(&mut self) -> bool {
        if self.can_run_heavy_test() {
            self.active_heavy_tests += 1;
            true
        } else {
            false
        }
    }

    fn release_heavy_test(&mut self) {
        if self.active_heavy_tests > 0 {
            self.active_heavy_tests -= 1;
        }
    }
}

/// Get the global test resource manager
fn get_resource_manager() -> Arc<Mutex<TestResourceManager>> {
    TEST_RESOURCE_MANAGER
        .get_or_init(|| Arc::new(Mutex::new(TestResourceManager::new())))
        .clone()
}

/// RAII guard for runtime resource management
pub struct RuntimeGuard {
    _runtime: Runtime,
}

impl RuntimeGuard {
    /// Creates a new single-threaded runtime with resource management
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = get_resource_manager();

        // Try to acquire runtime slot with timeout
        let start = Instant::now();
        let timeout = Duration::from_secs(30);

        loop {
            {
                let mut guard = manager.lock().unwrap();
                if guard.acquire_runtime() {
                    break;
                }
            }

            if start.elapsed() > timeout {
                return Err("Timeout waiting for runtime slot".into());
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        // Create single-threaded runtime to prevent TLS exhaustion
        let runtime = Builder::new_current_thread()
            .enable_all()
            .thread_stack_size(2 * 1024 * 1024) // 2MB stack
            .build()?;

        Ok(Self { _runtime: runtime })
    }

    /// Get a handle to the runtime
    pub fn runtime(&self) -> &Runtime {
        &self._runtime
    }
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        let manager = get_resource_manager();
        let mut guard = manager.lock().unwrap();
        guard.release_runtime();
    }
}

/// RAII guard for heavy test resource management
pub struct HeavyTestGuard;

impl HeavyTestGuard {
    /// Acquire a slot for running heavy tests
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = get_resource_manager();

        // Try to acquire heavy test slot with timeout
        let start = Instant::now();
        let timeout = Duration::from_secs(60);

        loop {
            {
                let mut guard = manager.lock().unwrap();
                if guard.acquire_heavy_test() {
                    break;
                }
            }

            if start.elapsed() > timeout {
                return Err("Timeout waiting for heavy test slot".into());
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        Ok(Self)
    }
}

impl Drop for HeavyTestGuard {
    fn drop(&mut self) {
        let manager = get_resource_manager();
        let mut guard = manager.lock().unwrap();
        guard.release_heavy_test();
    }
}

/// Isolated test environment with proper cleanup
pub struct IsolatedTestEnv {
    pub temp_dir: TempDir,
    pub runtime_guard: Option<RuntimeGuard>,
    pub heavy_test_guard: Option<HeavyTestGuard>,
}

impl IsolatedTestEnv {
    /// Create a new isolated test environment
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        Ok(Self {
            temp_dir,
            runtime_guard: None,
            heavy_test_guard: None,
        })
    }

    /// Create environment with async runtime support
    pub fn with_runtime() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let runtime_guard = Some(RuntimeGuard::new()?);

        Ok(Self {
            temp_dir,
            runtime_guard,
            heavy_test_guard: None,
        })
    }

    /// Create environment for heavy/resource-intensive tests
    pub fn with_heavy_test() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let heavy_test_guard = Some(HeavyTestGuard::new()?);

        Ok(Self {
            temp_dir,
            runtime_guard: None,
            heavy_test_guard,
        })
    }

    /// Create environment with both runtime and heavy test protection
    pub fn with_full_isolation() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let runtime_guard = Some(RuntimeGuard::new()?);
        let heavy_test_guard = Some(HeavyTestGuard::new()?);

        Ok(Self {
            temp_dir,
            runtime_guard,
            heavy_test_guard,
        })
    }

    /// Get the temporary directory path
    pub fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }

    /// Get the runtime if available
    pub fn runtime(&self) -> Option<&Runtime> {
        self.runtime_guard.as_ref().map(|g| g.runtime())
    }

    /// Execute async code with the managed runtime
    pub fn block_on<F, R>(&self, future: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        match &self.runtime_guard {
            Some(guard) => guard.runtime().block_on(future),
            None => panic!("Runtime not available - use with_runtime() or with_full_isolation()"),
        }
    }
}

/// Helper macros for test isolation

/// Run a test with basic isolation (temp dir only)
#[macro_export]
macro_rules! isolated_test {
    ($test_fn:expr) => {{
        let env = $crate::common::test_isolation::IsolatedTestEnv::new()
            .expect("Failed to create isolated test environment");
        $test_fn(env);
    }};
}

/// Run an async test with runtime isolation
#[macro_export]
macro_rules! isolated_async_test {
    ($test_fn:expr) => {{
        let env = $crate::common::test_isolation::IsolatedTestEnv::with_runtime()
            .expect("Failed to create isolated async test environment");
        $test_fn(env);
    }};
}

/// Run a heavy test with full resource isolation
#[macro_export]
macro_rules! isolated_heavy_test {
    ($test_fn:expr) => {{
        let env = $crate::common::test_isolation::IsolatedTestEnv::with_full_isolation()
            .expect("Failed to create isolated heavy test environment");
        $test_fn(env);
    }};
}

/// Test configuration helpers
pub struct TestConfig {
    pub max_threads: usize,
    pub timeout: Duration,
    pub memory_limit_mb: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            max_threads: 2, // Conservative default
            timeout: Duration::from_secs(30),
            memory_limit_mb: 256,
        }
    }
}

impl TestConfig {
    pub fn light() -> Self {
        Self {
            max_threads: 1,
            timeout: Duration::from_secs(10),
            memory_limit_mb: 64,
        }
    }

    pub fn medium() -> Self {
        Self::default()
    }

    pub fn heavy() -> Self {
        Self {
            max_threads: 4,
            timeout: Duration::from_secs(120),
            memory_limit_mb: 512,
        }
    }
}

/// Skip test if running in CI with resource constraints
pub fn skip_if_ci_resource_constrained() -> bool {
    // Check for common CI environment variables
    if std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
    {
        // Check if we're in a resource-constrained environment
        let manager = get_resource_manager();
        let guard = manager.lock().unwrap();

        // Skip if too many heavy tests are already running
        if !guard.can_run_heavy_test() || !guard.can_create_runtime() {
            eprintln!("Skipping test due to CI resource constraints");
            return true;
        }
    }

    false
}

/// Conditional test runner that respects resource limits
pub fn run_if_resources_available<F>(test_fn: F)
where
    F: FnOnce(),
{
    if skip_if_ci_resource_constrained() {
        eprintln!("Test skipped due to resource constraints");
        return;
    }

    test_fn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_env_creation() {
        let env = IsolatedTestEnv::new().unwrap();
        assert!(env.path().exists());
        assert!(env.runtime().is_none());
    }

    #[test]
    fn test_isolated_env_with_runtime() {
        let env = IsolatedTestEnv::with_runtime().unwrap();
        assert!(env.path().exists());
        assert!(env.runtime().is_some());
    }

    #[test]
    fn test_resource_manager_limits() {
        let manager = get_resource_manager();

        // Test runtime acquisition
        {
            let mut guard = manager.lock().unwrap();
            assert!(guard.can_create_runtime());
            assert!(guard.acquire_runtime());
            assert_eq!(guard.active_runtimes, 1);
            guard.release_runtime();
            assert_eq!(guard.active_runtimes, 0);
        }

        // Test heavy test acquisition
        {
            let mut guard = manager.lock().unwrap();
            assert!(guard.can_run_heavy_test());
            assert!(guard.acquire_heavy_test());
            assert_eq!(guard.active_heavy_tests, 1);
            guard.release_heavy_test();
            assert_eq!(guard.active_heavy_tests, 0);
        }
    }

    #[test]
    fn test_config_presets() {
        let light = TestConfig::light();
        assert_eq!(light.max_threads, 1);
        assert!(light.timeout < Duration::from_secs(30));

        let heavy = TestConfig::heavy();
        assert!(heavy.max_threads > 2);
        assert!(heavy.timeout > Duration::from_secs(60));
    }
}
