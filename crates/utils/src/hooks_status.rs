//! Hook status tracking and state management

use crate::paths::{ensure_status_dir_exists, get_hooks_status_file_path};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
        }
    }
}

/// Manager for hook status tracking
pub struct HooksStatusManager {
    status: Arc<Mutex<HooksStatus>>,
    status_file: PathBuf,
}

impl HooksStatusManager {
    /// Create a new status manager
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

    /// Initialize status for a set of hooks
    pub fn initialize_hooks(&self, hook_names: Vec<String>) -> io::Result<()> {
        let mut status = self.status.lock().unwrap();
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
        let mut status = self.status.lock().unwrap();
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
        self.status.lock().unwrap().clone()
    }

    /// Clear the status
    pub fn clear_status(&self) -> io::Result<()> {
        let mut status = self.status.lock().unwrap();
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
        let status = self.status.lock().unwrap();
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
        let status_file = get_hooks_status_file_path();
        if !status_file.exists() {
            return Ok(HooksStatus::default());
        }

        let content = fs::read_to_string(status_file)?;
        serde_json::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl Default for HooksStatusManager {
    fn default() -> Self {
        Self::new().expect("Failed to create status manager")
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

    #[test]
    fn test_status_manager_creation() {
        let manager = HooksStatusManager::new().unwrap();
        // Clear any existing status first
        let _ = manager.clear_status();

        let status = manager.get_current_status();
        assert_eq!(status.total, 0);
        assert_eq!(status.completed, 0);
        assert_eq!(status.failed, 0);
    }

    #[test]
    fn test_initialize_hooks() {
        // Ensure status directory exists
        let _ = ensure_status_dir_exists();

        let manager = HooksStatusManager::new().unwrap();
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
        let manager = HooksStatusManager::new().unwrap();
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
        let manager = HooksStatusManager::new().unwrap();
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
        // Create and populate manager
        {
            let manager = HooksStatusManager::new().unwrap();
            manager
                .initialize_hooks(vec!["persistent_hook".to_string()])
                .unwrap();
            manager.mark_hook_started("persistent_hook", 9999).unwrap();
        }

        // Read from file
        let status = HooksStatusManager::read_status_from_file().unwrap();
        assert_eq!(status.total, 1);
        assert!(status.hooks.contains_key("persistent_hook"));
        assert_eq!(status.hooks["persistent_hook"].status, HookState::Running);
    }

    #[test]
    fn test_clear_status() {
        let manager = HooksStatusManager::new().unwrap();
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
