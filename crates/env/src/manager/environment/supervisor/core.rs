//! Core supervisor implementation

use crate::manager::environment::interactive::{ControlFlow, InteractiveHandler};
use cuenv_config::Hook;
use cuenv_core::Result;
use cuenv_utils::directory_lock::DirectoryLock;
use cuenv_utils::hooks_status::{HookState, HooksStatusManager};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::cache;
use super::execution::execute_hook_with_timeout;
use super::utils::{get_cache_dir, is_process_running};

/// The mode in which the supervisor should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorMode {
    /// Run hooks interactively in the foreground, allowing user to background them.
    Foreground,
    /// Run hooks in the background without user interaction.
    Background,
    /// Run hooks synchronously, blocking until completion.
    Synchronous,
}

/// Preload hook supervisor that manages background hook execution
pub struct Supervisor {
    /// Hooks to execute
    hooks: Vec<Hook>,
    /// The mode in which the supervisor is running
    mode: SupervisorMode,
    /// Status manager for progress tracking
    status_manager: Arc<HooksStatusManager>,
    /// Handler for interactive terminal operations (optional)
    interactive_handler: Option<InteractiveHandler>,
    /// Directory lock (held for lifetime of supervisor)
    _lock: Option<DirectoryLock>,
    /// Directory for caching
    cache_dir: PathBuf,
}

impl Supervisor {
    /// Create a new preload supervisor
    pub fn new(hooks: Vec<Hook>, mode: SupervisorMode) -> Result<Self> {
        let status_manager = HooksStatusManager::new().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {e}"))
        })?;

        let cache_dir = get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| cuenv_core::Error::file_system(&cache_dir, "create directory", e))?;

        let status_manager_arc = Arc::new(status_manager);

        let interactive_handler = if mode == SupervisorMode::Foreground {
            Some(InteractiveHandler::with_status_manager(Arc::clone(
                &status_manager_arc,
            )))
        } else {
            None
        };

        Ok(Self {
            hooks,
            mode,
            status_manager: status_manager_arc,
            interactive_handler,
            _lock: None, // Legacy mode doesn't use locking
            cache_dir,
        })
    }

    /// Create a new preload supervisor for a specific directory
    pub fn new_for_directory(
        directory: &Path,
        hooks: Vec<Hook>,
        mode: SupervisorMode,
    ) -> Result<Self> {
        // Try to acquire directory lock if in foreground mode
        let lock = if matches!(mode, SupervisorMode::Foreground) {
            match DirectoryLock::try_acquire(directory) {
                Ok(lock) => Some(lock),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Hooks already running for this directory
                    return Err(cuenv_core::Error::configuration(
                        "Hooks already running for this directory",
                    ));
                }
                Err(e) => {
                    return Err(cuenv_core::Error::file_system(
                        directory,
                        "acquire directory lock",
                        e,
                    ));
                }
            }
        } else {
            None
        };

        // Create directory-specific status manager
        let status_manager = HooksStatusManager::new_for_directory(directory).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {e}"))
        })?;

        let cache_dir = cuenv_utils::paths::get_state_dir(directory);
        fs::create_dir_all(&cache_dir)
            .map_err(|e| cuenv_core::Error::file_system(&cache_dir, "create directory", e))?;

        let status_manager_arc = Arc::new(status_manager);

        let interactive_handler = if mode == SupervisorMode::Foreground {
            Some(InteractiveHandler::with_status_manager(Arc::clone(
                &status_manager_arc,
            )))
        } else {
            None
        };

        Ok(Self {
            hooks,
            mode,
            status_manager: status_manager_arc,
            interactive_handler,
            _lock: lock,
            cache_dir,
        })
    }

    /// Run the supervisor
    pub async fn run(mut self) -> Result<()> {
        match self.mode {
            SupervisorMode::Foreground => self.run_foreground().await,
            SupervisorMode::Background => self.run_background().await,
            SupervisorMode::Synchronous => self.run_synchronous().await,
        }
    }

    async fn run_foreground(&mut self) -> Result<()> {
        if self.hooks.is_empty() {
            return Ok(());
        }

        // Check if hooks are already running
        let current_status = self.status_manager.get_current_status();

        // Check for stale hooks (marked as running but process is dead)
        let mut has_stale_hooks = false;
        let mut has_actually_running_hooks = false;

        for hook in current_status.hooks.values() {
            if matches!(hook.status, HookState::Running | HookState::Pending) {
                if let Some(pid) = hook.pid {
                    if is_process_running(pid) {
                        has_actually_running_hooks = true;
                    } else {
                        has_stale_hooks = true;
                    }
                } else if matches!(hook.status, HookState::Pending) {
                    // Pending hooks don't have PIDs yet
                    has_actually_running_hooks = true;
                }
            }
        }

        if has_stale_hooks {
            eprintln!(
                "# cuenv: Detected stale hooks (processes no longer running), clearing status..."
            );
            self.status_manager.clear_status().map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to clear status: {e}"))
            })?;
        } else if has_actually_running_hooks {
            eprintln!("# cuenv: Hooks are already running, skipping...");
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = cache::calculate_input_hash(&self.hooks)?;
        if let Ok(cached_env) = cache::load_cached_environment(&self.cache_dir, &input_hash) {
            // Inputs haven't changed, use cached environment
            eprintln!("# cuenv: Using cached environment (inputs unchanged)");
            cache::apply_cached_environment(&self.cache_dir, cached_env)?;
            return Ok(());
        }

        eprintln!("# cuenv: Running {} hook(s)...", self.hooks.len());

        // Clear any stale status from previous runs before initializing new hooks
        self.status_manager.clear_status().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to clear status: {e}"))
        })?;

        // Initialize status tracking with all hooks
        let hook_names: Vec<String> = self
            .hooks
            .iter()
            .map(|h| {
                if let Some(args) = &h.args {
                    format!("{} {:?}", h.command, args)
                } else {
                    h.command.clone()
                }
            })
            .collect();

        self.status_manager
            .initialize_hooks(hook_names)
            .map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {e}"))
            })?;

        let mut handles = Vec::new();

        for hook in self.hooks.iter().cloned() {
            let status_manager = self.status_manager.clone();
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let handle = tokio::spawn(async move {
                // Start with no PID, will be updated when process spawns
                let _ = status_manager.mark_hook_started(&hook_key, 0);
                let result = execute_hook_with_timeout(&hook, Duration::from_secs(60), false).await;
                match result {
                    Ok((output, pid)) => {
                        // Update with actual PID if we got one
                        if let Some(actual_pid) = pid {
                            let _ = status_manager.mark_hook_started(&hook_key, actual_pid);
                        }
                        let _ = status_manager.mark_hook_completed(&hook_key);
                        output
                    }
                    Err(e) => {
                        let _ = status_manager.mark_hook_failed(&hook_key, e.to_string());
                        None
                    }
                }
            });
            handles.push(handle);
        }

        let mut message_shown = false;
        let start_time = std::time::Instant::now();

        loop {
            let all_finished = handles.iter().all(|h| h.is_finished());
            if all_finished {
                break;
            }

            // Show message after 1 second
            if !message_shown && start_time.elapsed() > Duration::from_secs(1) {
                eprintln!("# cuenv: Press 'b' to background, 'q' to quit");
                message_shown = true;
            }

            // Always check for input after message is shown
            if message_shown {
                if let Some(interactive_handler) = &mut self.interactive_handler {
                    if interactive_handler
                        .monitor_with_timeout(Duration::from_millis(200))
                        .await
                        == ControlFlow::Background
                    {
                        // Spawn a background task to monitor hook completion
                        eprintln!(
                            "# cuenv: Continuing {} hook(s) in background...",
                            handles.len()
                        );

                        let _status_manager = self.status_manager.clone();
                        let hooks = self.hooks.clone();
                        let cache_dir = self.cache_dir.clone();

                        tokio::spawn(async move {
                            // Wait for all handles to complete
                            let mut captured_env = HashMap::new();
                            for handle in handles {
                                if let Ok(Some(env)) = handle.await {
                                    captured_env.extend(env);
                                }
                            }

                            // Save captured environment if any
                            if !captured_env.is_empty() {
                                if let Ok(input_hash) = cache::calculate_input_hash(&hooks) {
                                    let _ = cache::save_cached_environment(
                                        &cache_dir,
                                        &input_hash,
                                        captured_env,
                                    );
                                }
                            }

                            // DON'T clear status - keep it available for status command
                            // The status will be cleared on next directory change
                        });

                        return Ok(());
                    }
                }
            } else {
                // Before message is shown, just sleep briefly
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let mut captured_env = HashMap::new();
        for handle in handles {
            if let Ok(Some(env)) = handle.await {
                captured_env.extend(env);
            }
        }

        if !captured_env.is_empty() {
            let input_hash = cache::calculate_input_hash(&self.hooks)?;
            cache::save_cached_environment(&self.cache_dir, &input_hash, captured_env)?;
        }

        self.status_manager.clear_status()?;
        eprintln!("# cuenv: ✓ All hooks completed");
        Ok(())
    }

    async fn run_synchronous(&self) -> Result<()> {
        self.run_silent_synchronous().await
    }

    async fn run_silent_synchronous(&self) -> Result<()> {
        if self.hooks.is_empty() {
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = cache::calculate_input_hash(&self.hooks)?;
        if let Ok(cached_env) = cache::load_cached_environment(&self.cache_dir, &input_hash) {
            // Inputs haven't changed, use cached environment
            cache::apply_cached_environment(&self.cache_dir, cached_env)?;
            return Ok(());
        }

        self.status_manager.clear_status().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to clear status: {e}"))
        })?;

        let hook_names: Vec<String> = self
            .hooks
            .iter()
            .map(|h| {
                if let Some(args) = &h.args {
                    format!("{} {:?}", h.command, args)
                } else {
                    h.command.clone()
                }
            })
            .collect();

        self.status_manager
            .initialize_hooks(hook_names)
            .map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {e}"))
            })?;

        let mut captured_env = HashMap::new();
        let mut handles = Vec::new();

        for hook in self.hooks.iter().cloned() {
            let status_manager = self.status_manager.clone();
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let handle = tokio::spawn(async move {
                let _ = status_manager.mark_hook_started(&hook_key, 0);
                let result = execute_hook_with_timeout(&hook, Duration::from_secs(60), true).await;
                match result {
                    Ok((output, pid)) => {
                        if let Some(actual_pid) = pid {
                            let _ = status_manager.mark_hook_started(&hook_key, actual_pid);
                        }
                        let _ = status_manager.mark_hook_completed(&hook_key);
                        output
                    }
                    Err(e) => {
                        let _ = status_manager.mark_hook_failed(&hook_key, e.to_string());
                        None
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Ok(Some(env)) = handle.await {
                captured_env.extend(env);
            }
        }

        if !captured_env.is_empty() {
            let input_hash = cache::calculate_input_hash(&self.hooks)?;
            cache::save_cached_environment(&self.cache_dir, &input_hash, captured_env)?;
        }

        self.status_manager.clear_status()?;
        Ok(())
    }

    async fn run_background(&self) -> Result<()> {
        self.execute_hooks_in_background().await
    }

    async fn execute_hooks_in_background(&self) -> Result<()> {
        if self.hooks.is_empty() {
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = cache::calculate_input_hash(&self.hooks)?;
        if let Ok(cached_env) = cache::load_cached_environment(&self.cache_dir, &input_hash) {
            // Inputs haven't changed, use cached environment
            cache::apply_cached_environment(&self.cache_dir, cached_env)?;
            return Ok(());
        }

        eprintln!(
            "# cuenv: Starting {} hook(s) in background...",
            self.hooks.len()
        );

        // Clear any stale status from previous runs
        self.status_manager.clear_status().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to clear status: {e}"))
        })?;

        // Initialize status tracking
        let hook_names: Vec<String> = self
            .hooks
            .iter()
            .map(|h| {
                if let Some(args) = &h.args {
                    format!("{} {:?}", h.command, args)
                } else {
                    h.command.clone()
                }
            })
            .collect();

        self.status_manager
            .initialize_hooks(hook_names)
            .map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {e}"))
            })?;

        // Collect environment from source hooks
        let mut captured_env = HashMap::new();

        // Execute all hooks
        let mut handles = Vec::new();
        for hook in self.hooks.iter().cloned() {
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let status_manager = self.status_manager.clone();
            let hook_clone = hook.clone();
            let hook_key_clone = hook_key.clone();

            eprintln!("# cuenv: Running hook: {}", hook.command);

            // Mark hook as started
            let pid = std::process::id();
            status_manager
                .mark_hook_started(&hook_key, pid)
                .map_err(|e| {
                    cuenv_core::Error::configuration(format!("Failed to update status: {e}"))
                })?;

            let handle = tokio::spawn(async move {
                let result =
                    execute_hook_with_timeout(&hook_clone, Duration::from_secs(60), false).await;

                match result {
                    Ok((output, pid)) => {
                        // Update with actual PID if we got one
                        if let Some(actual_pid) = pid {
                            let _ = status_manager.mark_hook_started(&hook_key_clone, actual_pid);
                        }
                        eprintln!("# cuenv: Hook completed: {}", hook_clone.command);
                        let _ = status_manager.mark_hook_completed(&hook_key_clone);

                        // Return environment if this was a source hook
                        output
                    }
                    Err(e) => {
                        eprintln!("# cuenv: Hook failed: {}: {}", hook_clone.command, e);
                        let _ = status_manager.mark_hook_failed(&hook_key_clone, e.to_string());
                        None
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all hooks to complete
        for handle in handles {
            if let Ok(Some(env)) = handle.await {
                captured_env.extend(env);
            }
        }

        // Save captured environment if any
        if !captured_env.is_empty() {
            let input_hash = cache::calculate_input_hash(&self.hooks)?;
            cache::save_cached_environment(&self.cache_dir, &input_hash, captured_env)?;
        }

        // Clear status after successful completion
        self.status_manager.clear_status()?;
        eprintln!("# cuenv: ✓ All hooks completed");
        Ok(())
    }
}
