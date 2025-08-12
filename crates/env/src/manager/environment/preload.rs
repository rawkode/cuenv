use cuenv_config::Hook;
use cuenv_core::Result;
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
}

/// Manages preload hooks that run in the background
#[derive(Clone)]
pub struct PreloadHookManager {
    inner: Arc<PreloadHookManagerInner>,
}

impl PreloadHookManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(PreloadHookManagerInner {
                running_hooks: Mutex::new(HashMap::new()),
                timeout: DEFAULT_PRELOAD_TIMEOUT,
            }),
        }
    }

    #[allow(dead_code)]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            inner: Arc::new(PreloadHookManagerInner {
                running_hooks: Mutex::new(HashMap::new()),
                timeout,
            }),
        }
    }

    /// Execute preload hooks in the background
    pub async fn execute_preload_hooks(&self, hooks: Vec<Hook>) -> Result<()> {
        let mut running = self.inner.running_hooks.lock().await;

        for hook in hooks {
            if hook.preload.unwrap_or(false) && !hook.source.unwrap_or(false) {
                let hook_key = format!("{} {:?}", hook.command, hook.args);
                let hook_clone = hook.clone();

                tracing::info!("Starting preload hook in background: {}", hook_key);

                let handle = tokio::spawn(async move {
                    if let Err(e) = execute_hook_async(&hook_clone).await {
                        tracing::warn!("Preload hook failed: {}: {}", hook_clone.command, e);
                    } else {
                        tracing::info!("Preload hook completed: {}", hook_clone.command);
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
    let mut cmd = tokio::process::Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    let status = cmd.status().await.map_err(|e| {
        cuenv_core::Error::command_execution(
            hook.command.clone(),
            hook.args.clone().unwrap_or_default(),
            format!("Failed to execute preload hook: {e}"),
            None,
        )
    })?;

    if !status.success() {
        tracing::warn!("Preload hook failed with status: {:?}", status.code());
    }

    Ok(())
}

impl Default for PreloadHookManager {
    fn default() -> Self {
        Self::new()
    }
}
