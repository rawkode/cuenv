//! Input validation utilities for CUE parsing

use cuenv_core::constants::{CUENV_PACKAGE_VAR, DEFAULT_PACKAGE_NAME};
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

    // Get the expected package name from environment or use default
    let expected_package =
        std::env::var(CUENV_PACKAGE_VAR).unwrap_or_else(|_| DEFAULT_PACKAGE_NAME.to_string());

    // Only allow loading the configured package
    if package_name != expected_package {
        return Err(Error::configuration(format!(
            "Only '{expected_package}' package is supported, got '{package_name}'. Please ensure your .cue files use 'package {expected_package}'"
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
    use std::env;

    #[test]
    fn test_validate_package_name() {
        // Set up test environment
        let original = env::var(CUENV_PACKAGE_VAR).ok();
        env::set_var(CUENV_PACKAGE_VAR, "testpkg");

        // Empty package name should fail
        assert!(validate_package_name("").is_err());

        // Non-matching package should fail
        assert!(validate_package_name("mypackage").is_err());

        // Configured package should succeed
        assert!(validate_package_name("testpkg").is_ok());

        // Restore original value
        if let Some(val) = original {
            env::set_var(CUENV_PACKAGE_VAR, val);
        } else {
            env::remove_var(CUENV_PACKAGE_VAR);
        }
    }

    #[test]
    fn test_validate_package_name_default() {
        // Remove the env var to test default
        let original = env::var(CUENV_PACKAGE_VAR).ok();
        env::remove_var(CUENV_PACKAGE_VAR);

        // Default package should succeed
        assert!(validate_package_name(DEFAULT_PACKAGE_NAME).is_ok());

        // Non-default package should fail
        assert!(validate_package_name("notdefault").is_err());

        // Restore original value
        if let Some(val) = original {
            env::set_var(CUENV_PACKAGE_VAR, val);
        }
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
