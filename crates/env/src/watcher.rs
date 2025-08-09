use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, trace};

/// Tracks file modifications for cache invalidation
pub struct FileWatcher {
    paths: Vec<PathBuf>,
    last_modified: HashMap<PathBuf, SystemTime>,
}

impl FileWatcher {
    /// Create a new file watcher for the given paths
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let mut last_modified = HashMap::new();

        for path in &paths {
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    last_modified.insert(path.clone(), modified);
                    trace!(
                        "Tracking file: {} (modified: {:?})",
                        path.display(),
                        modified
                    );
                }
            }
        }

        debug!("Created file watcher for {} files", paths.len());

        FileWatcher {
            paths,
            last_modified,
        }
    }

    /// Check if any watched files have been modified
    pub fn needs_reload(&self) -> bool {
        for path in &self.paths {
            if self.file_modified(path) {
                debug!("File {} has been modified, reload needed", path.display());
                return true;
            }
        }
        false
    }

    /// Check if a specific file has been modified
    fn file_modified(&self, path: &Path) -> bool {
        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Some(&last) = self.last_modified.get(path) {
                    return modified > last;
                }
                // File is new (wasn't tracked before)
                return true;
            }
        }

        // File doesn't exist anymore or can't read metadata
        // If we were tracking it before, consider it modified
        self.last_modified.contains_key(path)
    }

    /// Update the last modified times for all watched files
    pub fn update_timestamps(&mut self) {
        self.last_modified.clear();

        for path in &self.paths {
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    self.last_modified.insert(path.clone(), modified);
                }
            }
        }
    }

    /// Add a new path to watch
    pub fn add_path(&mut self, path: PathBuf) {
        if !self.paths.contains(&path) {
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    self.last_modified.insert(path.clone(), modified);
                    self.paths.push(path);
                }
            }
        }
    }

    /// Get the list of watched paths
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Check if a cache file is newer than all watched files
    pub fn cache_is_valid(&self, cache_path: &Path) -> bool {
        // Get cache file modification time
        let cache_modified = match cache_path.metadata() {
            Ok(metadata) => match metadata.modified() {
                Ok(modified) => modified,
                Err(_) => return false,
            },
            Err(_) => return false, // Cache doesn't exist
        };

        // Check if any watched file is newer than cache
        for path in &self.paths {
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified > cache_modified {
                        debug!("File {} is newer than cache, invalidating", path.display());
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// Get default files to watch for a project directory
pub fn default_watch_files(project_dir: &Path) -> Vec<PathBuf> {
    let mut files = vec![project_dir.join("env.cue"), project_dir.join(".envrc")];

    // Add flake files if they exist
    let flake_nix = project_dir.join("flake.nix");
    if flake_nix.exists() {
        files.push(flake_nix);
        files.push(project_dir.join("flake.lock"));
    }

    // Add shell.nix if it exists
    let shell_nix = project_dir.join("shell.nix");
    if shell_nix.exists() {
        files.push(shell_nix);
    }

    // Add default.nix if it exists
    let default_nix = project_dir.join("default.nix");
    if default_nix.exists() {
        files.push(default_nix);
    }

    // Add devenv files if they exist
    let devenv_nix = project_dir.join("devenv.nix");
    if devenv_nix.exists() {
        files.push(devenv_nix);
        files.push(project_dir.join("devenv.lock"));
        files.push(project_dir.join("devenv.yaml"));
    }

    // Add home config files
    if let Ok(home) = std::env::var("HOME") {
        let home_path = Path::new(&home);
        files.push(home_path.join(".config/cuenv/config.cue"));
        files.push(home_path.join(".cuenvrc"));
    }

    // Filter to only existing files
    files.into_iter().filter(|p| p.exists()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_watcher_detects_changes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create initial file
        fs::write(&file_path, "initial content").unwrap();

        // Create watcher
        let watcher = FileWatcher::new(vec![file_path.clone()]);

        // Initially no reload needed
        assert!(!watcher.needs_reload());

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&file_path, "modified content").unwrap();

        // Now reload is needed
        assert!(watcher.needs_reload());
    }

    #[test]
    fn test_cache_validity() {
        let temp_dir = TempDir::new().unwrap();
        let source_file = temp_dir.path().join("source.txt");
        let cache_file = temp_dir.path().join("cache.txt");

        // Create source file
        fs::write(&source_file, "source").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create cache file (newer than source)
        fs::write(&cache_file, "cache").unwrap();

        let watcher = FileWatcher::new(vec![source_file.clone()]);

        // Cache should be valid (newer than source)
        assert!(watcher.cache_is_valid(&cache_file));

        // Modify source file
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&source_file, "modified source").unwrap();

        // Cache should now be invalid
        assert!(!watcher.cache_is_valid(&cache_file));
    }
}
