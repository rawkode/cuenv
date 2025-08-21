use cuenv_config::Hook;
use cuenv_core::Result;
use cuenv_utils::hooks_status::HooksStatusManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Default timeout for preload hooks (60 seconds)
const DEFAULT_PRELOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Inner state for PreloadHookManager
struct PreloadHookManagerInner {
    /// Running preload hooks (hook command -> join handle)
    running_hooks: Mutex<HashMap<String, JoinHandle<()>>>,
    /// Timeout for preload hooks
    timeout: Duration,
    /// Status manager for progress tracking
    status_manager: Option<Arc<HooksStatusManager>>,
}

/// Manages preload hooks that run in the background
#[derive(Clone)]
pub struct PreloadHookManager {
    inner: Arc<PreloadHookManagerInner>,
}

impl PreloadHookManager {
    pub fn new() -> Self {
        // Try to create a status manager for progress tracking
        let status_manager = match HooksStatusManager::new() {
            Ok(mgr) => Some(Arc::new(mgr)),
            Err(e) => {
                tracing::warn!("Failed to create status manager: {}", e);
                None
            }
        };

        Self {
            inner: Arc::new(PreloadHookManagerInner {
                running_hooks: Mutex::new(HashMap::new()),
                timeout: DEFAULT_PRELOAD_TIMEOUT,
                status_manager,
            }),
        }
    }

    #[allow(dead_code)]
    pub fn with_timeout(timeout: Duration) -> Self {
        // Try to create a status manager for progress tracking
        let status_manager = match HooksStatusManager::new() {
            Ok(mgr) => Some(Arc::new(mgr)),
            Err(e) => {
                tracing::warn!("Failed to create status manager: {}", e);
                None
            }
        };

        Self {
            inner: Arc::new(PreloadHookManagerInner {
                running_hooks: Mutex::new(HashMap::new()),
                timeout,
                status_manager,
            }),
        }
    }

    /// Execute preload hooks in the background
    pub async fn execute_preload_hooks(&self, hooks: Vec<Hook>) -> Result<()> {
        let mut running = self.inner.running_hooks.lock().await;

        // Count preload hooks
        let preload_count = hooks.iter().filter(|h| h.preload.unwrap_or(false)).count();

        if preload_count > 0 {
            tracing::error!("# cuenv: Starting {preload_count} preload hook(s) in background...");
        }

        // Initialize status tracking if available
        if let Some(ref status_manager) = self.inner.status_manager {
            let hook_names: Vec<String> = hooks
                .iter()
                .filter(|h| h.preload.unwrap_or(false))
                .map(|h| {
                    if let Some(args) = &h.args {
                        format!("{} {:?}", h.command, args)
                    } else {
                        h.command.clone()
                    }
                })
                .collect();

            if !hook_names.is_empty() {
                let _ = status_manager.initialize_hooks(hook_names);
            }
        }

        for hook in hooks {
            if hook.preload.unwrap_or(false) {
                let hook_key = if let Some(args) = &hook.args {
                    format!("{} {:?}", hook.command, args)
                } else {
                    hook.command.clone()
                };
                let hook_clone = hook.clone();
                let status_manager = self.inner.status_manager.clone();

                tracing::info!("Starting preload hook in background: {}", hook_key);
                tracing::error!("# cuenv: Running preload hook: {}", hook.command);

                // Mark hook as started
                if let Some(ref sm) = status_manager {
                    let pid = std::process::id();
                    let _ = sm.mark_hook_started(&hook_key, pid);
                }

                let hook_key_clone = hook_key.clone();
                let handle = tokio::spawn(async move {
                    let result = execute_hook_async(&hook_clone).await;

                    // Update status based on result
                    if let Some(ref sm) = status_manager {
                        match &result {
                            Ok(_) => {
                                tracing::info!("Preload hook completed: {}", hook_clone.command);
                                tracing::error!(
                                    "# cuenv: Preload hook completed: {}",
                                    hook_clone.command
                                );
                                let _ = sm.mark_hook_completed(&hook_key_clone);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Preload hook failed: {}: {}",
                                    hook_clone.command,
                                    e
                                );
                                tracing::error!(
                                    "# cuenv: Preload hook failed: {}: {}",
                                    hook_clone.command,
                                    e
                                );
                                let _ = sm.mark_hook_failed(&hook_key_clone, e.to_string());
                            }
                        }
                    } else {
                        // Fallback logging if no status manager
                        match result {
                            Ok(_) => {
                                tracing::info!("Preload hook completed: {}", hook_clone.command);
                                tracing::error!(
                                    "# cuenv: Preload hook completed: {}",
                                    hook_clone.command
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Preload hook failed: {}: {}",
                                    hook_clone.command,
                                    e
                                );
                                tracing::error!(
                                    "# cuenv: Preload hook failed: {}: {}",
                                    hook_clone.command,
                                    e
                                );
                            }
                        }
                    }
                });

                running.insert(hook_key, handle);
            }
        }

        Ok(())
    }

    /// Wait for all preload hooks to complete
    pub async fn wait_for_completion(&self) -> Result<()> {
        let mut running = self.inner.running_hooks.lock().await;

        if running.is_empty() {
            return Ok(());
        }

        tracing::info!("Waiting for {} preload hooks to complete...", running.len());

        let mut handles = Vec::new();
        for (key, handle) in running.drain() {
            handles.push((key, handle));
        }

        // Release the lock before waiting
        drop(running);

        for (key, handle) in handles {
            match timeout(self.inner.timeout, handle).await {
                Ok(Ok(())) => {
                    tracing::debug!("Preload hook completed: {}", key);
                }
                Ok(Err(e)) => {
                    tracing::warn!("Preload hook panicked: {}: {}", key, e);
                }
                Err(_) => {
                    tracing::warn!(
                        "Preload hook timed out after {:?}: {}",
                        self.inner.timeout,
                        key
                    );
                }
            }
        }

        Ok(())
    }

    /// Cancel all running preload hooks
    pub async fn cancel_all(&self) {
        let mut running = self.inner.running_hooks.lock().await;

        if !running.is_empty() {
            tracing::info!("Canceling {} running preload hooks", running.len());

            for (key, handle) in running.drain() {
                handle.abort();
                tracing::debug!("Canceled preload hook: {}", key);
            }

            // Clear status tracking
            if let Some(ref status_manager) = self.inner.status_manager {
                let _ = status_manager.clear_status();
            }
        }
    }

    /// Check if any preload hooks are running
    pub async fn has_running_hooks(&self) -> bool {
        let running = self.inner.running_hooks.lock().await;
        !running.is_empty()
    }

    /// Get status of running hooks
    pub async fn get_status(&self) -> Vec<String> {
        let running = self.inner.running_hooks.lock().await;
        running.keys().cloned().collect()
    }
}

/// Execute a hook asynchronously
async fn execute_hook_async(hook: &Hook) -> Result<()> {
    use std::process::Stdio;

    let mut cmd = tokio::process::Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // Capture all output to prevent it from appearing in terminal
    // For preload hooks, we want to completely silence all output
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    if hook.source.unwrap_or(false) {
        // For source hooks, we need to capture and parse the output
        let output = cmd.output().await.map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute preload hook: {e}"),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Preload source hook failed: {}", stderr);
        } else {
            // Parse and apply environment variables
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Try to parse environment variables from output
            if let Ok(env_vars) = crate::source_parser::evaluate_shell_environment(&stdout) {
                let filtered = crate::source_parser::filter_environment(env_vars);

                // Apply the environment variables to the current process
                // Note: This only affects the cuenv process, not the parent shell
                // The shell hook mechanism will handle propagating these to the shell
                for (key, value) in filtered {
                    std::env::set_var(&key, &value);
                    tracing::debug!("Set env var from preload source hook: {}={}", key, value);
                }
            }
        }
    } else {
        // For non-source hooks, just run and discard output
        let output = cmd.output().await.map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute preload hook: {e}"),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Preload hook failed: {}", stderr);
        }
    }

    Ok(())
}

impl Default for PreloadHookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::Hook;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_hook(command: &str, args: Option<Vec<String>>, preload: bool) -> Hook {
        Hook {
            command: command.to_string(),
            args,
            dir: None,
            inputs: None,
            source: None,
            preload: Some(preload),
        }
    }

    fn create_source_hook(command: &str, args: Option<Vec<String>>, preload: bool) -> Hook {
        Hook {
            command: command.to_string(),
            args,
            dir: None,
            inputs: None,
            source: Some(true),
            preload: Some(preload),
        }
    }

    #[tokio::test]
    async fn test_preload_manager_creation() {
        let manager = PreloadHookManager::new();
        assert!(!manager.has_running_hooks().await);
        assert_eq!(manager.get_status().await.len(), 0);
    }

    #[tokio::test]
    async fn test_preload_manager_with_custom_timeout() {
        let custom_timeout = Duration::from_secs(30);
        let manager = PreloadHookManager::with_timeout(custom_timeout);
        assert_eq!(manager.inner.timeout, custom_timeout);
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_empty_hooks_list() {
        let manager = PreloadHookManager::new();
        let result = manager.execute_preload_hooks(vec![]).await;
        assert!(result.is_ok());
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_non_preload_hooks() {
        let manager = PreloadHookManager::new();
        let hooks = vec![
            create_test_hook("echo", Some(vec!["hello".to_string()]), false),
            create_test_hook("pwd", None, false),
        ];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_simple_preload_hook() {
        let manager = PreloadHookManager::new();
        let hooks = vec![create_test_hook(
            "echo",
            Some(vec!["test".to_string()]),
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());
        assert!(manager.has_running_hooks().await);

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_multiple_preload_hooks() {
        let manager = PreloadHookManager::new();
        let hooks = vec![
            create_test_hook("echo", Some(vec!["test1".to_string()]), true),
            create_test_hook("echo", Some(vec!["test2".to_string()]), true),
            create_test_hook("pwd", None, true),
        ];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());
        assert!(manager.has_running_hooks().await);

        let status = manager.get_status().await;
        assert_eq!(status.len(), 3);

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_mixed_preload_and_regular_hooks() {
        let manager = PreloadHookManager::new();
        let hooks = vec![
            create_test_hook("echo", Some(vec!["preload".to_string()]), true),
            create_test_hook("echo", Some(vec!["regular".to_string()]), false),
            create_test_hook("pwd", None, true),
        ];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Only preload hooks should be running
        let status = manager.get_status().await;
        assert_eq!(status.len(), 2);

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_execute_source_hook() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("test_script.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'export TEST_VAR=test_value'",
        )
        .unwrap();

        // Make script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }

        let manager = PreloadHookManager::new();
        let hooks = vec![create_source_hook(
            script_path.to_string_lossy().as_ref(),
            None,
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_hook_with_directory() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PreloadHookManager::new();

        let mut hook = create_test_hook("pwd", None, true);
        hook.dir = Some(temp_dir.path().to_string_lossy().to_string());

        let result = manager.execute_preload_hooks(vec![hook]).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_failing_hook() {
        let manager = PreloadHookManager::new();
        let hooks = vec![create_test_hook("false", None, true)]; // Command that always fails

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok()); // Execute should succeed even if hook fails

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_nonexistent_command() {
        let manager = PreloadHookManager::new();
        let hooks = vec![create_test_hook("nonexistent_command_xyz123", None, true)];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok()); // Execute should succeed even if hook fails

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_hook_timeout() {
        let short_timeout = Duration::from_millis(100);
        let manager = PreloadHookManager::with_timeout(short_timeout);

        // Create a hook that sleeps longer than the timeout
        let hooks = vec![create_test_hook("sleep", Some(vec!["1".to_string()]), true)];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion - should timeout
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok()); // Timeout is handled gracefully
    }

    #[tokio::test]
    async fn test_cancel_all_hooks() {
        let manager = PreloadHookManager::new();

        // Start some long-running hooks
        let hooks = vec![
            create_test_hook("sleep", Some(vec!["10".to_string()]), true),
            create_test_hook("sleep", Some(vec!["10".to_string()]), true),
        ];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());
        assert!(manager.has_running_hooks().await);

        // Cancel all hooks
        manager.cancel_all().await;
        assert!(!manager.has_running_hooks().await);
    }

    #[tokio::test]
    async fn test_concurrent_hook_execution() {
        let manager = Arc::new(PreloadHookManager::new());

        let manager1 = Arc::clone(&manager);
        let manager2 = Arc::clone(&manager);

        let task1 = tokio::spawn(async move {
            let hooks = vec![create_test_hook(
                "echo",
                Some(vec!["task1".to_string()]),
                true,
            )];
            manager1.execute_preload_hooks(hooks).await
        });

        let task2 = tokio::spawn(async move {
            let hooks = vec![create_test_hook(
                "echo",
                Some(vec!["task2".to_string()]),
                true,
            )];
            manager2.execute_preload_hooks(hooks).await
        });

        let (result1, result2) = tokio::join!(task1, task2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());

        // Wait for all hooks to complete
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_hook_key_generation() {
        let manager = PreloadHookManager::new();

        let hooks = vec![
            create_test_hook("echo", Some(vec!["hello".to_string()]), true),
            create_test_hook("echo", None, true),
            create_test_hook("pwd", Some(vec!["-L".to_string()]), true),
        ];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        let status = manager.get_status().await;
        assert_eq!(status.len(), 3);

        // Check that different hooks generate different keys
        assert!(status.contains(&"echo [\"hello\"]".to_string()));
        assert!(status.contains(&"echo".to_string()));
        assert!(status.contains(&"pwd [\"-L\"]".to_string()));

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_duplicate_hook_handling() {
        let manager = PreloadHookManager::new();

        // Execute the same hook twice
        let hook = create_test_hook("echo", Some(vec!["duplicate".to_string()]), true);
        let hooks = vec![hook.clone(), hook];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // The second hook should overwrite the first one due to same key
        let status = manager.get_status().await;
        assert_eq!(status.len(), 1);

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_shell_specific_hooks() {
        let manager = PreloadHookManager::new();

        // Test bash-specific command
        let hooks = vec![create_test_hook(
            "bash",
            Some(vec!["-c".to_string(), "echo 'bash test'".to_string()]),
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_environment_variable_conflicts() {
        let manager = PreloadHookManager::new();

        // Set initial environment variable
        std::env::set_var("CUENV_TEST_CONFLICT", "initial");

        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("conflict_script.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'export CUENV_TEST_CONFLICT=modified'",
        )
        .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }

        let hooks = vec![create_source_hook(
            script_path.to_string_lossy().as_ref(),
            None,
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());

        // Clean up
        std::env::remove_var("CUENV_TEST_CONFLICT");
    }

    #[tokio::test]
    async fn test_large_number_of_preload_hooks() {
        let manager = PreloadHookManager::new();

        // Create many hooks to test performance
        let mut hooks = Vec::new();
        for i in 0..50 {
            hooks.push(create_test_hook(
                "echo",
                Some(vec![format!("hook_{}", i)]),
                true,
            ));
        }

        let start = std::time::Instant::now();
        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());

        let duration = start.elapsed();
        // Should complete within reasonable time (less than 30 seconds)
        assert!(duration.as_secs() < 30);
    }

    #[tokio::test]
    async fn test_hook_with_complex_arguments() {
        let manager = PreloadHookManager::new();

        let hooks = vec![create_test_hook(
            "echo",
            Some(vec![
                "complex".to_string(),
                "arguments".to_string(),
                "with spaces".to_string(),
                "--flag=value".to_string(),
            ]),
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Wait for completion
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_status_tracking_without_status_manager() {
        // Create manager that might fail to create status manager
        let manager = PreloadHookManager::new();

        let hooks = vec![create_test_hook(
            "echo",
            Some(vec!["status_test".to_string()]),
            true,
        )];

        let result = manager.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Should still work without status manager
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_completion_with_no_hooks() {
        let manager = PreloadHookManager::new();

        // Should return immediately if no hooks are running
        let wait_result = manager.wait_for_completion().await;
        assert!(wait_result.is_ok());
    }

    #[tokio::test]
    async fn test_manager_cloning() {
        let manager1 = PreloadHookManager::new();
        let manager2 = manager1.clone();

        // Start hook in one manager
        let hooks = vec![create_test_hook(
            "echo",
            Some(vec!["clone_test".to_string()]),
            true,
        )];
        let result = manager1.execute_preload_hooks(hooks).await;
        assert!(result.is_ok());

        // Should be visible in cloned manager
        assert!(manager2.has_running_hooks().await);

        // Wait for completion via cloned manager
        let wait_result = manager2.wait_for_completion().await;
        assert!(wait_result.is_ok());

        // Both managers should show no running hooks
        assert!(!manager1.has_running_hooks().await);
        assert!(!manager2.has_running_hooks().await);
    }
}
