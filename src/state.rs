use crate::env_diff::EnvDiff;
use crate::file_times::FileTimes;
use crate::gzenv;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// State information stored in environment variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuenvState {
    /// The directory containing the loaded env.cue file
    pub dir: PathBuf,
    /// The path to the loaded env.cue file
    pub file: PathBuf,
    /// The environment being used (e.g., "dev", "staging", "production")
    pub environment: Option<String>,
    /// The capabilities that were loaded
    pub capabilities: Vec<String>,
}

/// Manage cuenv state through environment variables
pub struct StateManager;

impl StateManager {
    /// Get the environment variable name with optional prefix
    pub(crate) fn env_var_name(base: &str) -> String {
        if let Ok(prefix) = env::var("CUENV_PREFIX") {
            format!("{prefix}_{base}")
        } else {
            base.to_string()
        }
    }

    /// Check if an environment is currently loaded
    pub fn is_loaded() -> bool {
        env::var(Self::env_var_name("CUENV_DIR")).is_ok()
    }

    /// Get the currently loaded directory
    pub fn current_dir() -> Option<PathBuf> {
        env::var(Self::env_var_name("CUENV_DIR"))
            .ok()
            .and_then(|dir| {
                // Remove the leading '-' that direnv uses
                dir.strip_prefix('-')
                    .map(PathBuf::from)
                    .or_else(|| Some(PathBuf::from(dir)))
            })
    }

    /// Load state for a directory
    pub fn load(
        dir: &Path,
        file: &Path,
        environment: Option<&str>,
        capabilities: &[String],
        diff: &EnvDiff,
        watches: &FileTimes,
    ) -> Result<()> {
        // Set CUENV_DIR with leading '-' like direnv
        env::set_var(
            Self::env_var_name("CUENV_DIR"),
            format!("-{}", dir.display()),
        );

        // Set CUENV_FILE
        env::set_var(Self::env_var_name("CUENV_FILE"), file.display().to_string());

        // Set CUENV_DIFF
        let diff_encoded = gzenv::encode(diff).context("Failed to encode environment diff")?;
        env::set_var(Self::env_var_name("CUENV_DIFF"), diff_encoded);

        // Set CUENV_WATCHES
        let watches_encoded = gzenv::encode(watches).context("Failed to encode file watches")?;
        env::set_var(Self::env_var_name("CUENV_WATCHES"), watches_encoded);

        // Store additional state
        let state = CuenvState {
            dir: dir.to_path_buf(),
            file: file.to_path_buf(),
            environment: environment.map(|s| s.to_string()),
            capabilities: capabilities.to_vec(),
        };

        let state_encoded = gzenv::encode(&state).context("Failed to encode state")?;
        env::set_var(Self::env_var_name("CUENV_STATE"), state_encoded);

        Ok(())
    }

    /// Unload the current environment
    pub fn unload() -> Result<()> {
        // Get the diff to revert changes
        if let Ok(diff_encoded) = env::var(Self::env_var_name("CUENV_DIFF")) {
            if let Ok(diff) = gzenv::decode::<EnvDiff>(&diff_encoded) {
                // Apply the reverse diff to restore original environment
                diff.reverse().apply();
            }
        }

        // Remove cuenv state variables
        env::remove_var(Self::env_var_name("CUENV_DIR"));
        env::remove_var(Self::env_var_name("CUENV_FILE"));
        env::remove_var(Self::env_var_name("CUENV_DIFF"));
        env::remove_var(Self::env_var_name("CUENV_WATCHES"));
        env::remove_var(Self::env_var_name("CUENV_STATE"));

        Ok(())
    }

    /// Get the current state
    pub fn get_state() -> Result<Option<CuenvState>> {
        match env::var(Self::env_var_name("CUENV_STATE")) {
            Ok(encoded) => {
                let state = gzenv::decode(&encoded).context("Failed to decode state")?;
                Ok(Some(state))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get the environment diff
    pub fn get_diff() -> Result<Option<EnvDiff>> {
        match env::var(Self::env_var_name("CUENV_DIFF")) {
            Ok(encoded) => {
                let diff = gzenv::decode(&encoded).context("Failed to decode diff")?;
                Ok(Some(diff))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get the file watches
    pub fn get_watches() -> Result<Option<FileTimes>> {
        match env::var(Self::env_var_name("CUENV_WATCHES")) {
            Ok(encoded) => {
                let watches = gzenv::decode(&encoded).context("Failed to decode watches")?;
                Ok(Some(watches))
            }
            Err(_) => Ok(None),
        }
    }

    /// Check if watched files have changed
    pub fn files_changed() -> bool {
        if let Ok(Some(watches)) = Self::get_watches() {
            watches.has_changed()
        } else {
            false
        }
    }

    /// Check if we should load environment for a directory
    pub fn should_load(dir: &Path) -> bool {
        match Self::current_dir() {
            Some(current) => current != dir,
            None => true,
        }
    }

    /// Check if we should unload when leaving a directory
    pub fn should_unload(current_dir: &Path) -> bool {
        if let Some(loaded_dir) = Self::current_dir() {
            // Unload if we're no longer in the loaded directory or its subdirectories
            !current_dir.starts_with(&loaded_dir)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Mutex to ensure state tests don't interfere with each other
    static STATE_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    #[serial]
    fn test_state_management() {
        let _lock = STATE_TEST_MUTEX.lock().unwrap();

        // Use a unique prefix for this test with thread ID to avoid race conditions
        let thread_id = std::thread::current().id();
        let test_prefix = format!("TEST_STATE_MGMT_{:?}", thread_id);
        env::set_var("CUENV_PREFIX", &test_prefix);

        // Clean environment - remove any cuenv state variables
        env::remove_var(StateManager::env_var_name("CUENV_DIR"));
        env::remove_var(StateManager::env_var_name("CUENV_FILE"));
        env::remove_var(StateManager::env_var_name("CUENV_DIFF"));
        env::remove_var(StateManager::env_var_name("CUENV_WATCHES"));
        env::remove_var(StateManager::env_var_name("CUENV_STATE"));

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();
        let file = dir.join("env.cue");

        // Initially not loaded
        assert!(!StateManager::is_loaded());
        assert!(StateManager::current_dir().is_none());

        // Create a diff
        let mut prev = HashMap::new();
        prev.insert("OLD_VAR".to_string(), "old".to_string());
        let mut next = HashMap::new();
        next.insert("NEW_VAR".to_string(), "new".to_string());
        let diff = EnvDiff::new(prev, next);

        // Create watches
        let watches = FileTimes::new();

        // Load state
        StateManager::load(
            dir,
            &file,
            Some("dev"),
            &["cap1".to_string()],
            &diff,
            &watches,
        )
        .unwrap();

        // Check loaded state
        // Debug: check if the env var is actually set
        let cuenv_dir_var = StateManager::env_var_name("CUENV_DIR");
        let cuenv_dir_value = env::var(&cuenv_dir_var);
        assert!(
            cuenv_dir_value.is_ok(),
            "CUENV_DIR env var not set: {}",
            cuenv_dir_var
        );

        assert!(StateManager::is_loaded());

        // Compare canonical paths to handle symlinks and path resolution differences
        let current_dir = StateManager::current_dir().unwrap();
        let expected_dir = dir.to_path_buf();

        // In build environments, paths might be resolved differently, so compare canonical forms
        let current_canonical = current_dir.canonicalize().unwrap_or(current_dir);
        let expected_canonical = expected_dir.canonicalize().unwrap_or(expected_dir);

        assert_eq!(current_canonical, expected_canonical);

        // Get state
        let state = StateManager::get_state().unwrap().unwrap();

        // Compare canonical paths to handle symlinks and path resolution differences
        let state_dir_canonical = state.dir.canonicalize().unwrap_or(state.dir.clone());
        let expected_dir_canonical = dir.canonicalize().unwrap_or(dir.to_path_buf());
        assert_eq!(state_dir_canonical, expected_dir_canonical);

        assert_eq!(state.file, file);
        assert_eq!(state.environment, Some("dev".to_string()));
        assert_eq!(state.capabilities, vec!["cap1".to_string()]);

        // Check diff
        let loaded_diff = StateManager::get_diff().unwrap().unwrap();
        assert_eq!(loaded_diff, diff);

        // Unload
        StateManager::unload().unwrap();
        assert!(!StateManager::is_loaded());

        // Clean up our specific environment variables before removing prefix
        env::remove_var(StateManager::env_var_name("CUENV_DIR"));
        env::remove_var(StateManager::env_var_name("CUENV_FILE"));
        env::remove_var(StateManager::env_var_name("CUENV_DIFF"));
        env::remove_var(StateManager::env_var_name("CUENV_WATCHES"));
        env::remove_var(StateManager::env_var_name("CUENV_STATE"));

        // Finally remove the prefix
        env::remove_var("CUENV_PREFIX");
    }

    #[test]
    #[serial]
    fn test_should_load_unload() {
        let _lock = STATE_TEST_MUTEX.lock().unwrap();

        // Use a unique prefix for this test with thread ID to avoid race conditions
        let thread_id = std::thread::current().id();
        let test_prefix = format!("TEST_SHOULD_LOAD_{:?}", thread_id);
        env::set_var("CUENV_PREFIX", &test_prefix);

        let temp_dir = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        let root = temp_dir.path();
        let subdir = root.join("subdir");
        let other = temp_dir2.path(); // Use a completely different temp directory

        // Clean state
        env::remove_var(StateManager::env_var_name("CUENV_DIR"));
        env::remove_var(StateManager::env_var_name("CUENV_FILE"));
        env::remove_var(StateManager::env_var_name("CUENV_DIFF"));
        env::remove_var(StateManager::env_var_name("CUENV_WATCHES"));
        env::remove_var(StateManager::env_var_name("CUENV_STATE"));

        // Create subdirectory
        fs::create_dir_all(&subdir).unwrap();

        // Should load when nothing is loaded
        assert!(StateManager::should_load(root));

        // Simulate loading root
        env::set_var(
            StateManager::env_var_name("CUENV_DIR"),
            format!("-{}", root.display()),
        );

        // Should not reload same directory
        assert!(!StateManager::should_load(root));

        // Should load different directory
        assert!(StateManager::should_load(&other));

        // Should not unload when in subdirectory
        assert!(!StateManager::should_unload(&subdir));

        // Should unload when leaving to different directory
        assert!(StateManager::should_unload(&other));

        // Clean up our specific environment variables before removing prefix
        env::remove_var(StateManager::env_var_name("CUENV_DIR"));
        env::remove_var(StateManager::env_var_name("CUENV_FILE"));
        env::remove_var(StateManager::env_var_name("CUENV_DIFF"));
        env::remove_var(StateManager::env_var_name("CUENV_WATCHES"));
        env::remove_var(StateManager::env_var_name("CUENV_STATE"));

        // Finally remove the prefix
        env::remove_var("CUENV_PREFIX");
    }
}
