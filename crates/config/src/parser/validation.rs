//! Input validation utilities for CUE parsing

use cuenv_core::constants::ENV_PACKAGE_NAME;
use cuenv_core::errors::{Error, Result};
use std::ffi::CString;
use std::path::Path;

/// Validates that the package name is allowed
pub fn validate_package_name(package_name: &str) -> Result<()> {
    if package_name.is_empty() {
        return Err(Error::configuration(
            "Package name cannot be empty".to_string(),
        ));
    }

    // Only allow loading the "env" package
    if package_name != ENV_PACKAGE_NAME {
        return Err(Error::configuration(format!(
            "Only 'env' package is supported, got '{package_name}'. Please ensure your .cue files use 'package env'"
        )));
    }

    Ok(())
}

/// Validates and converts a directory path to a string
pub fn validate_directory_path(dir: &Path) -> Result<String> {
    let dir_str = dir.to_string_lossy();
    if dir_str.is_empty() {
        return Err(Error::configuration(
            "Directory path cannot be empty".to_string(),
        ));
    }
    Ok(dir_str.to_string())
}

/// Creates a CString for FFI, ensuring no null bytes
pub fn create_ffi_string(value: &str, context: &str) -> Result<CString> {
    CString::new(value).map_err(|e| {
        Error::ffi(
            "cue_eval_package",
            format!("{context} - contains null byte: {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_package_name() {
        // Empty package name should fail
        assert!(validate_package_name("").is_err());

        // Non-env package should fail
        assert!(validate_package_name("mypackage").is_err());

        // Only "env" package should succeed
        assert!(validate_package_name("env").is_ok());
    }

    #[test]
    fn test_validate_directory_path() {
        // Empty path should fail
        assert!(validate_directory_path(Path::new("")).is_err());

        // Valid path should succeed
        let result = validate_directory_path(Path::new("/tmp/test"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/tmp/test");
    }
}
