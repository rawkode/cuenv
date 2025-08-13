//! Path utilities for cuenv-specific file locations

use std::path::PathBuf;
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
