use crate::errors::{Error, Result};
use std::env;
use std::path::PathBuf;

pub struct DirectoryManager;

impl DirectoryManager {
    pub fn new() -> Self {
        Self
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
