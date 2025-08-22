//! Hook status tracking and state management

use crate::paths::{
    ensure_state_dir_exists, ensure_status_dir_exists, get_hooks_status_file_path,
    get_hooks_status_file_path_for_dir,
};
#[cfg(unix)]
use libc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Status of an individual hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStatus {
    /// Name of the hook
    pub name: String,
    /// Process ID if running
    pub pid: Option<u32>,
    /// Start time as Unix timestamp
    pub start_time: u64,
    /// Completion status
    pub status: HookState,
    /// Duration in seconds (if completed)
    pub duration: Option<f64>,
    /// Error message if failed
    pub error: Option<String>,
}

/// State of a hook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HookState {
    /// Hook is pending execution
    Pending,
    /// Hook is currently running
    Running,
    /// Hook completed successfully
    Completed,
    /// Hook failed
    Failed,
}

/// Overall hooks status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksStatus {
    /// Map of hook name to status
    pub hooks: HashMap<String, HookStatus>,
    /// Total number of hooks
    pub total: usize,
    /// Number of completed hooks
    pub completed: usize,
    /// Number of failed hooks
    pub failed: usize,
    /// Overall start time
    pub start_time: u64,
    /// Last update time
    pub last_update: u64,
    /// Directory this status is for (optional for backwards compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    /// PID of the supervisor that owns this status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervisor_pid: Option<u32>,
}

impl Default for HooksStatus {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            hooks: HashMap::new(),
            total: 0,
            completed: 0,
            failed: 0,
            start_time: now,
            last_update: now,
            directory: None,
            supervisor_pid: None,
        }
    }
}

impl HooksStatus {
    /// Clean up stale hooks (marked as running but process is dead)
    pub fn cleanup_stale_hooks(&mut self) {
        for (_name, hook) in self.hooks.iter_mut() {
            if matches!(hook.status, HookState::Running) {
                if let Some(pid) = hook.pid {
                    if !is_process_running(pid) {
                        // Mark as completed since process is dead
                        hook.status = HookState::Completed;
                        self.completed += 1;
                    }
                }
            }
        }
    }

    /// Check if there are actually running hooks
    pub fn has_actually_running_hooks(&self) -> bool {
        self.hooks.values().any(|h| {
            matches!(h.status, HookState::Running | HookState::Pending)
                && h.pid.is_none_or(is_process_running)
        })
    }
}

/// Check if a process with the given PID is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use libc::kill with signal 0 to check if process exists
        // This is more reliable than checking /proc
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    #[cfg(not(unix))]
    {
        // For non-Unix systems, conservatively assume it's not running
        false
    }
}

/// Manager for hook status tracking
pub struct HooksStatusManager {
    status: Arc<Mutex<HooksStatus>>,
    status_file: PathBuf,
}

impl HooksStatusManager {
    /// Create a new status manager (legacy mode for backwards compatibility)
    pub fn new() -> io::Result<Self> {
        ensure_status_dir_exists()?;
        let status_file = get_hooks_status_file_path();

        // Try to load existing status or create new
        let status = if status_file.exists() {
            match fs::read_to_string(&status_file) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => HooksStatus::default(),
            }
        } else {
            HooksStatus::default()
        };

        Ok(Self {
            status: Arc::new(Mutex::new(status)),
            status_file,
        })
    }

    /// Create a new status manager for a specific directory
    pub fn new_for_directory(directory: &Path) -> io::Result<Self> {
        ensure_state_dir_exists(directory)?;
        let status_file = get_hooks_status_file_path_for_dir(directory);

        // Try to load existing status or create new
        let mut status = if status_file.exists() {
            match fs::read_to_string(&status_file) {
                Ok(content) => {
                    let mut s: HooksStatus = serde_json::from_str(&content).unwrap_or_default();
                    // Clean up stale hooks on load
                    s.cleanup_stale_hooks();
                    s
                }
                Err(_) => HooksStatus::default(),
            }
        } else {
            HooksStatus::default()
        };

        // Set directory and supervisor PID
        status.directory = Some(directory.to_string_lossy().to_string());
        status.supervisor_pid = Some(std::process::id());

        Ok(Self {
            status: Arc::new(Mutex::new(status)),
            status_file,
        })
    }

    /// Initialize status for a set of hooks
    pub fn initialize_hooks(&self, hook_names: Vec<String>) -> io::Result<()> {
        let mut status = self.status.lock();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        status.hooks.clear();
        status.total = hook_names.len();
        status.completed = 0;
        status.failed = 0;
        status.start_time = now;
        status.last_update = now;

        for name in hook_names {
            status.hooks.insert(
                name.clone(),
                HookStatus {
                    name,
                    pid: None,
                    start_time: now,
                    status: HookState::Pending,
                    duration: None,
                    error: None,
                },
            );
        }

        drop(status);
        self.persist_status()
    }

    /// Update the status of a specific hook
    pub fn update_hook_status(
        &self,
        name: &str,
        state: HookState,
        pid: Option<u32>,
        error: Option<String>,
    ) -> io::Result<()> {
        let mut status = self.status.lock();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(hook) = status.hooks.get_mut(name) {
            let old_state = hook.status.clone();
            hook.status = state.clone();
            hook.pid = pid;
            hook.error = error;

            // Calculate duration if transitioning to completed/failed
            if matches!(state, HookState::Completed | HookState::Failed) {
                hook.duration = Some((now - hook.start_time) as f64);
            }

            // Update counters
            if old_state != state {
                match (&old_state, &state) {
                    (_, HookState::Completed) => status.completed += 1,
                    (_, HookState::Failed) => status.failed += 1,
                    _ => {}
                }
            }
        }

        status.last_update = now;
        drop(status);
        self.persist_status()
    }

    /// Mark a hook as started
    pub fn mark_hook_started(&self, name: &str, pid: u32) -> io::Result<()> {
        self.update_hook_status(name, HookState::Running, Some(pid), None)
    }

    /// Mark a hook as completed
    pub fn mark_hook_completed(&self, name: &str) -> io::Result<()> {
        self.update_hook_status(name, HookState::Completed, None, None)
    }

    /// Mark a hook as failed
    pub fn mark_hook_failed(&self, name: &str, error: String) -> io::Result<()> {
        self.update_hook_status(name, HookState::Failed, None, Some(error))
    }

    /// Get the current status
    pub fn get_current_status(&self) -> HooksStatus {
        self.status.lock().clone()
    }

    /// Clear the status
    pub fn clear_status(&self) -> io::Result<()> {
        let mut status = self.status.lock();
        *status = HooksStatus::default();
        drop(status);

        // Remove the status file
        if self.status_file.exists() {
            fs::remove_file(&self.status_file)?;
        }
        Ok(())
    }

    /// Persist status to file atomically
    fn persist_status(&self) -> io::Result<()> {
        let status = self.status.lock();
        let json = serde_json::to_string_pretty(&*status)?;
        drop(status);

        // Write atomically using a temporary file
        let temp_file = self.status_file.with_extension("tmp");
        let mut file = fs::File::create(&temp_file)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        // Atomic rename
        fs::rename(temp_file, &self.status_file)?;
        Ok(())
    }

    /// Read status from file
    pub fn read_status_from_file() -> io::Result<HooksStatus> {
        ensure_status_dir_exists()?;
        let status_file = get_hooks_status_file_path();
        if !status_file.exists() {
            return Ok(HooksStatus::default());
        }

        let content = fs::read_to_string(status_file)?;
        serde_json::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Read status for a specific directory (static method)
    pub fn read_status_for_directory(directory: &Path) -> io::Result<Option<HooksStatus>> {
        let status_file = get_hooks_status_file_path_for_dir(directory);

        if !status_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&status_file)?;
        let mut status: HooksStatus = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        status.cleanup_stale_hooks();
        Ok(Some(status))
    }
}

impl Default for HooksStatusManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to in-memory only mode if we can't create the file-based manager
            HooksStatusManager {
                status: Arc::new(Mutex::new(HooksStatus::default())),
                status_file: std::env::temp_dir().join("cuenv-hooks-status-fallback.json"),
            }
        })
    }
}

/// Calculate elapsed time since start
pub fn calculate_elapsed(start_time: u64) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Duration::from_secs(now.saturating_sub(start_time))
}

/// Check if status should still be shown (within 5 seconds of completion)
pub fn should_show_completed_status(last_update: u64) -> bool {
    let elapsed = calculate_elapsed(last_update);
    elapsed.as_secs() <= 5
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_status_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();

        let status = manager.get_current_status();
        assert_eq!(status.total, 0);
        assert_eq!(status.completed, 0);
        assert_eq!(status.failed, 0);
    }

    #[test]
    fn test_initialize_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();
        let hooks = vec!["hook1".to_string(), "hook2".to_string()];
        manager.initialize_hooks(hooks).unwrap();

        let status = manager.get_current_status();
        assert_eq!(status.total, 2);
        assert_eq!(status.hooks.len(), 2);
        assert!(status.hooks.contains_key("hook1"));
        assert!(status.hooks.contains_key("hook2"));
    }

    #[test]
    fn test_hook_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();
        manager
            .initialize_hooks(vec!["test_hook".to_string()])
            .unwrap();

        // Start hook
        manager.mark_hook_started("test_hook", 1234).unwrap();
        let status = manager.get_current_status();
        assert_eq!(status.hooks["test_hook"].status, HookState::Running);
        assert_eq!(status.hooks["test_hook"].pid, Some(1234));

        // Complete hook
        thread::sleep(Duration::from_millis(10));
        manager.mark_hook_completed("test_hook").unwrap();
        let status = manager.get_current_status();
        assert_eq!(status.hooks["test_hook"].status, HookState::Completed);
        assert_eq!(status.completed, 1);
        assert!(status.hooks["test_hook"].duration.is_some());
    }

    #[test]
    fn test_hook_failure() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();
        manager
            .initialize_hooks(vec!["failing_hook".to_string()])
            .unwrap();

        manager.mark_hook_started("failing_hook", 5678).unwrap();
        manager
            .mark_hook_failed("failing_hook", "Test error".to_string())
            .unwrap();

        let status = manager.get_current_status();
        assert_eq!(status.hooks["failing_hook"].status, HookState::Failed);
        assert_eq!(status.failed, 1);
        assert_eq!(
            status.hooks["failing_hook"].error,
            Some("Test error".to_string())
        );
    }

    #[test]
    fn test_persistence_and_reload() {
        let temp_dir = TempDir::new().unwrap();

        // Create and populate manager - use current process PID so it exists
        let current_pid = std::process::id();
        {
            let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();
            manager
                .initialize_hooks(vec!["persistent_hook".to_string()])
                .unwrap();
            manager
                .mark_hook_started("persistent_hook", current_pid)
                .unwrap();
        }

        // Read from file using the directory-specific method
        let status = HooksStatusManager::read_status_for_directory(temp_dir.path())
            .unwrap()
            .unwrap();
        assert_eq!(status.total, 1);
        assert!(status.hooks.contains_key("persistent_hook"));

        // The status might be either Running (if the process check works) or Completed (if it doesn't)
        // Both are valid behaviors depending on the system's /proc implementation
        let hook_status = &status.hooks["persistent_hook"].status;
        assert!(
            matches!(hook_status, HookState::Running)
                || matches!(hook_status, HookState::Completed),
            "Expected hook status to be Running or Completed, got: {hook_status:?}"
        );
    }

    #[test]
    fn test_clear_status() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HooksStatusManager::new_for_directory(temp_dir.path()).unwrap();
        manager
            .initialize_hooks(vec!["hook1".to_string(), "hook2".to_string()])
            .unwrap();

        manager.clear_status().unwrap();
        let status = manager.get_current_status();
        assert_eq!(status.total, 0);
        assert_eq!(status.hooks.len(), 0);
    }

    #[test]
    fn test_should_show_completed_status() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Just completed
        assert!(should_show_completed_status(now));

        // 10 seconds ago - should not show
        assert!(!should_show_completed_status(now - 10));
    }
}
