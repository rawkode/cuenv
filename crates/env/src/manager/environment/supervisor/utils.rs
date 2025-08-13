//! Utility functions for the supervisor

use cuenv_core::Result;
use std::env;
use std::path::PathBuf;

/// Check if a process with the given PID is running
pub fn is_process_running(pid: u32) -> bool {
    // If it's the current process, it's definitely running
    if pid == std::process::id() {
        return true;
    }

    #[cfg(unix)]
    {
        // Try using kill with signal 0 to check if process exists
        // This is more reliable than checking /proc which might not be available in sandboxes
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    #[cfg(not(unix))]
    {
        // For non-Unix systems, conservatively assume it's running
        // This prevents accidental lock stealing
        true
    }
}

/// Get the cache directory for preload environments
pub fn get_cache_dir() -> Result<PathBuf> {
    // Use proper temp directory instead of hardcoded /tmp
    let temp_dir = env::temp_dir();
    let user = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let cache_dir = temp_dir.join(format!("cuenv-{user}")).join("preload-cache");
    Ok(cache_dir)
}