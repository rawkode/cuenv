use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct DirectoryManager {
    env_file_name: String,
}

impl DirectoryManager {
    pub fn new() -> Self {
        Self {
            env_file_name: "env.cue".to_string(),
        }
    }

    pub fn find_env_files(&self, start_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut env_files = Vec::new();
        let mut current = start_dir.to_path_buf();

        loop {
            let env_file = current.join(&self.env_file_name);
            if env_file.exists() && env_file.is_file() {
                env_files.push(env_file);
            }

            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => break,
            }
        }

        env_files.reverse();
        Ok(env_files)
    }

    pub fn get_current_directory() -> Result<PathBuf> {
        env::current_dir().context("Failed to get current directory")
    }

    pub fn should_load_env(&self, dir: &Path) -> bool {
        dir.join(&self.env_file_name).exists()
    }

    pub fn find_all_env_files_recursive(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut env_files = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.file_name() == self.env_file_name.as_str() {
                env_files.push(entry.path().to_path_buf());
            }
        }

        Ok(env_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_env_files() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub");
        fs::create_dir(&sub_dir).unwrap();

        fs::write(temp_dir.path().join("env.cue"), "{}").unwrap();
        fs::write(sub_dir.join("env.cue"), "{}").unwrap();

        let manager = DirectoryManager::new();
        let files = manager.find_env_files(&sub_dir).unwrap();

        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("env.cue"));
        assert!(files[1].parent().unwrap().ends_with("sub"));
    }
}