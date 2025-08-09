use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileTime {
    pub path: PathBuf,
    pub mtime: Option<SystemTime>,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileTimes {
    files: HashMap<PathBuf, FileTime>,
}

impl FileTimes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to watch
    pub fn watch(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        let file_time = Self::get_file_time(&path);
        self.files.insert(path, file_time);
    }

    /// Remove a file from watching
    pub fn unwatch(&mut self, path: impl AsRef<Path>) {
        self.files.remove(path.as_ref());
    }

    /// Check if any watched files have changed
    pub fn has_changed(&self) -> bool {
        for (path, old_time) in &self.files {
            let current_time = Self::get_file_time(path);

            // Check if existence changed
            if old_time.exists != current_time.exists {
                return true;
            }

            // Check if modification time changed
            if old_time.mtime != current_time.mtime {
                return true;
            }
        }

        false
    }

    /// Update all file times to current values
    pub fn update(&mut self) {
        let paths: Vec<_> = self.files.keys().cloned().collect();
        for path in paths {
            let file_time = Self::get_file_time(&path);
            self.files.insert(path, file_time);
        }
    }

    /// Get the list of files that have changed
    pub fn changed_files(&self) -> Vec<&Path> {
        let mut changed = Vec::new();

        for (path, old_time) in &self.files {
            let current_time = Self::get_file_time(path);

            if old_time.exists != current_time.exists || old_time.mtime != current_time.mtime {
                changed.push(path.as_path());
            }
        }

        changed
    }

    /// Clear all watched files
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Get the number of watched files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if watching any files
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn get_file_time(path: &Path) -> FileTime {
        match fs::metadata(path) {
            Ok(metadata) => FileTime {
                path: path.to_path_buf(),
                mtime: metadata.modified().ok(),
                exists: true,
            },
            Err(_) => FileTime {
                path: path.to_path_buf(),
                mtime: None,
                exists: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_watching() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create a file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Hello").unwrap();
        drop(file);

        // Start watching
        let mut times = FileTimes::new();
        times.watch(&file_path);

        // Initially no changes
        assert!(!times.has_changed());
        assert_eq!(times.changed_files().len(), 0);

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "World").unwrap();
        drop(file);

        // Should detect change
        assert!(times.has_changed());
        assert_eq!(times.changed_files().len(), 1);

        // Update times
        times.update();
        assert!(!times.has_changed());

        // Delete the file
        fs::remove_file(&file_path).unwrap();
        assert!(times.has_changed());
    }

    #[test]
    fn test_non_existent_file() {
        let mut times = FileTimes::new();
        let path = PathBuf::from("/non/existent/file");

        times.watch(&path);
        assert!(!times.has_changed());

        // Still shouldn't report changes for non-existent file
        assert!(!times.has_changed());
    }
}
