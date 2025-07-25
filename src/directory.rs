use crate::errors::{Error, Result};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub struct DirectoryManager;

impl DirectoryManager {
    pub fn new() -> Self {
        Self
    }

    pub fn allow_directory(&self, dir: &Path) -> Result<()> {
        let allowed_file = self.get_allowed_file()?;

        // Ensure the directory exists
        if !dir.exists() {
            return Err(Error::file_system(
                dir.to_path_buf(),
                "access directory",
                std::io::Error::new(std::io::ErrorKind::NotFound, "Directory does not exist"),
            ));
        }

        // Get canonical path
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| Error::file_system(dir.to_path_buf(), "canonicalize path", e))?;

        // Check if already allowed
        if self.is_directory_allowed(&canonical_dir)? {
            return Ok(()); // Already allowed
        }

        // Append to allowed file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&allowed_file)
            .map_err(|e| Error::file_system(allowed_file.clone(), "open allowed file", e))?;

        writeln!(file, "{}", canonical_dir.display())
            .map_err(|e| Error::file_system(allowed_file, "write to allowed file", e))?;

        Ok(())
    }

    pub fn deny_directory(&self, dir: &Path) -> Result<()> {
        let allowed_file = self.get_allowed_file()?;

        if !allowed_file.exists() {
            return Ok(()); // Nothing to deny
        }

        // Get canonical path
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| Error::file_system(dir.to_path_buf(), "canonicalize path", e))?;

        // Read all allowed directories
        let file = fs::File::open(&allowed_file)
            .map_err(|e| Error::file_system(allowed_file.clone(), "open allowed file", e))?;
        let reader = BufReader::new(file);

        let mut allowed_dirs: Vec<String> = Vec::new();
        for line in reader.lines() {
            let line =
                line.map_err(|e| Error::file_system(allowed_file.clone(), "read allowed file", e))?;
            let line = line.trim();
            if !line.is_empty() && line != canonical_dir.to_string_lossy() {
                allowed_dirs.push(line.to_string());
            }
        }

        // Write back the filtered list
        fs::write(&allowed_file, allowed_dirs.join("\n") + "\n")
            .map_err(|e| Error::file_system(allowed_file, "write allowed file", e))?;

        Ok(())
    }

    pub fn is_directory_allowed(&self, dir: &Path) -> Result<bool> {
        let allowed_file = self.get_allowed_file()?;

        if !allowed_file.exists() {
            return Ok(false);
        }

        // Get canonical path
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| Error::file_system(dir.to_path_buf(), "canonicalize path", e))?;

        // Read allowed directories
        let file = fs::File::open(&allowed_file)
            .map_err(|e| Error::file_system(allowed_file.clone(), "open allowed file", e))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line =
                line.map_err(|e| Error::file_system(allowed_file.clone(), "read allowed file", e))?;
            if line.trim() == canonical_dir.to_string_lossy() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn get_allowed_file(&self) -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| {
                Error::configuration("Could not determine config directory".to_string())
            })?
            .join("cuenv");

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| {
                Error::file_system(config_dir.clone(), "create config directory", e)
            })?;
        }

        Ok(config_dir.join("allowed"))
    }
}

impl Default for DirectoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectoryManager {
    pub fn get_current_directory() -> Result<PathBuf> {
        match env::current_dir() {
            Ok(dir) => Ok(dir),
            Err(e) => Err(Error::file_system(
                PathBuf::from("."),
                "get current directory",
                e,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_directory() {
        let result = DirectoryManager::get_current_directory();
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }
}
