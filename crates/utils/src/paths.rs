//! Path utilities for cuenv-specific file locations

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Get the path to the hooks status file
///
/// Returns the platform-appropriate path for storing hooks status:
/// - Unix: `/tmp/cuenv-$USER/hooks-status.json`
/// - Windows: `%TEMP%\cuenv-$USER\hooks-status.json`
pub fn get_hooks_status_file_path() -> PathBuf {
    let user = env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());

    #[cfg(unix)]
    {
        PathBuf::from(format!("/tmp/cuenv-{user}/hooks-status.json"))
    }

    #[cfg(windows)]
    {
        let temp_dir = env::temp_dir();
        temp_dir
            .join(format!("cuenv-{}", user))
            .join("hooks-status.json")
    }

    #[cfg(not(any(unix, windows)))]
    {
        let temp_dir = env::temp_dir();
        temp_dir
            .join(format!("cuenv-{}", user))
            .join("hooks-status.json")
    }
}

/// Ensure the status directory exists
///
/// Creates the parent directory for the status file if it doesn't exist
pub fn ensure_status_dir_exists() -> std::io::Result<()> {
    let status_file = get_hooks_status_file_path();
    if let Some(parent) = status_file.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Get the directory for cuenv temporary files
pub fn get_cuenv_temp_dir() -> PathBuf {
    let user = env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());

    #[cfg(unix)]
    {
        PathBuf::from(format!("/tmp/cuenv-{user}"))
    }

    #[cfg(windows)]
    {
        let temp_dir = env::temp_dir();
        temp_dir.join(format!("cuenv-{}", user))
    }

    #[cfg(not(any(unix, windows)))]
    {
        let temp_dir = env::temp_dir();
        temp_dir.join(format!("cuenv-{}", user))
    }
}

/// Generate a hash for a directory path to create unique state directories
pub fn get_directory_hash(dir: &Path) -> String {
    let mut hasher = Sha256::new();

    // Use canonical path to handle symlinks consistently
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    hasher.update(canonical.to_string_lossy().as_bytes());

    // Take first 16 chars of hex for reasonable length
    let full_hash = format!("{:x}", hasher.finalize());
    full_hash.chars().take(16).collect()
}

/// Get the state directory for a specific directory
pub fn get_state_dir(directory: &Path) -> PathBuf {
    let base_dir = get_cuenv_temp_dir();
    let dir_hash = get_directory_hash(directory);

    base_dir.join("state").join(dir_hash)
}

/// Get the hooks status file path for a specific directory
pub fn get_hooks_status_file_path_for_dir(directory: &Path) -> PathBuf {
    get_state_dir(directory).join("hooks_status.json")
}

/// Get the supervisor lock file path for a specific directory
pub fn get_supervisor_lock_path(directory: &Path) -> PathBuf {
    get_state_dir(directory).join("supervisor.lock")
}

/// Get the captured environment file path for a specific directory
pub fn get_captured_env_path(directory: &Path) -> PathBuf {
    get_state_dir(directory).join("captured_env.json")
}

/// Ensure the state directory exists for a specific directory
pub fn ensure_state_dir_exists(directory: &Path) -> std::io::Result<()> {
    let state_dir = get_state_dir(directory);
    fs::create_dir_all(state_dir)?;
    Ok(())
}

/// Get all active state directories (directories with hook status)
pub fn get_all_state_dirs() -> std::io::Result<Vec<(PathBuf, PathBuf)>> {
    let base_dir = get_cuenv_temp_dir();
    let state_root = base_dir.join("state");

    let mut result = Vec::new();

    if state_root.exists() {
        for entry in fs::read_dir(&state_root)? {
            let entry = entry?;
            let state_dir = entry.path();

            // Check if this state dir has a status file
            let status_file = state_dir.join("hooks_status.json");
            if status_file.exists() {
                // Try to determine original directory from a marker file
                // For now, we'll just return the state dir
                result.push((state_dir.clone(), state_dir));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_status_file_path() {
        let path = get_hooks_status_file_path();
        assert!(path.to_string_lossy().contains("cuenv"));
        assert!(path.to_string_lossy().contains("hooks-status.json"));
    }

    #[test]
    fn test_ensure_status_dir_exists() {
        // This should not fail even if the directory already exists
        assert!(ensure_status_dir_exists().is_ok());
    }

    #[test]
    fn test_cuenv_temp_dir() {
        let path = get_cuenv_temp_dir();
        assert!(path.to_string_lossy().contains("cuenv"));
    }
}
