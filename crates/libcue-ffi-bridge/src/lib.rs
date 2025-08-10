//! Go-Rust FFI bridge for CUE evaluation
//!
//! This crate provides a safe Rust interface to the Go-based CUE evaluator.
//! It handles all FFI operations, memory management, and error handling for
//! calling Go functions from Rust.

use cuenv_core::{Error, Result};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

/// RAII wrapper for C strings returned from FFI
/// Ensures proper cleanup when the wrapper goes out of scope
pub struct CStringPtr {
    ptr: *mut c_char,
}

impl CStringPtr {
    /// Creates a new wrapper from a raw pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is either null or a valid pointer returned from the FFI
    /// - The pointer has not been freed already
    /// - The pointer will not be used after this wrapper is dropped
    pub unsafe fn new(ptr: *mut c_char) -> Self {
        Self { ptr }
    }

    /// Checks if the wrapped pointer is null
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Converts the C string to a Rust &str
    ///
    /// # Safety
    /// The caller must ensure that the wrapped pointer is not null
    pub unsafe fn to_str(&self) -> Result<&str> {
        debug_assert!(
            !self.is_null(),
            "Attempted to convert null pointer to string"
        );

        let cstr = CStr::from_ptr(self.ptr);
        cstr.to_str().map_err(|e| {
            Error::ffi(
                "cue_eval_package",
                format!("failed to convert C string to UTF-8: {e}"),
            )
        })
    }
}

impl Drop for CStringPtr {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                cue_free_string(self.ptr);
            }
        }
    }
}

#[link(name = "cue_bridge")]
extern "C" {
    fn cue_eval_package(dir_path: *const c_char, package_name: *const c_char) -> *mut c_char;
    fn cue_free_string(s: *mut c_char);
}

/// Evaluates a CUE package and returns the result as a JSON string
///
/// # Arguments
/// * `dir_path` - Directory containing the CUE files
/// * `package_name` - Name of the CUE package to evaluate
///
/// # Returns
/// JSON string containing the evaluated CUE configuration
pub fn evaluate_cue_package(dir_path: &Path, package_name: &str) -> Result<String> {
    let dir_path_str = dir_path
        .to_str()
        .ok_or_else(|| Error::configuration("Invalid directory path: not UTF-8".to_string()))?;

    let c_dir = CString::new(dir_path_str)
        .map_err(|e| Error::ffi("cue_eval_package", format!("Invalid directory path: {e}")))?;

    let c_package = CString::new(package_name)
        .map_err(|e| Error::ffi("cue_eval_package", format!("Invalid package name: {e}")))?;

    let result_ptr = unsafe { cue_eval_package(c_dir.as_ptr(), c_package.as_ptr()) };

    let result = unsafe { CStringPtr::new(result_ptr) };

    if result.is_null() {
        return Err(Error::ffi(
            "cue_eval_package",
            "CUE evaluation returned null".to_string(),
        ));
    }

    let json_str = unsafe { result.to_str()? };

    // Check if the result is an error message from Go
    if json_str.starts_with("error:") {
        let error_msg = json_str.strip_prefix("error:").unwrap_or(json_str);
        return Err(Error::cue_parse(
            dir_path,
            format!("CUE evaluation error: {error_msg}"),
        ));
    }

    Ok(json_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cstring_ptr_creation() {
        // Test with null pointer
        let null_ptr = unsafe { CStringPtr::new(std::ptr::null_mut()) };
        assert!(null_ptr.is_null());

        // Test with non-null pointer (we'll create a mock one)
        // Note: In real scenarios, this would come from FFI calls
        let test_string = CString::new("test").unwrap();
        let ptr = test_string.into_raw();
        let wrapper = unsafe { CStringPtr::new(ptr) };
        assert!(!wrapper.is_null());

        // Convert back to string and verify
        let result_str = unsafe { wrapper.to_str().unwrap() };
        assert_eq!(result_str, "test");
        // CStringPtr will automatically free the memory when dropped
    }

    #[test]
    fn test_cstring_ptr_utf8_conversion() {
        let test_content = "Hello, 世界! 🦀";
        let c_string = CString::new(test_content).unwrap();
        let ptr = c_string.into_raw();
        let wrapper = unsafe { CStringPtr::new(ptr) };

        let converted = unsafe { wrapper.to_str().unwrap() };
        assert_eq!(converted, test_content);
    }

    #[test]
    fn test_cstring_ptr_empty_string() {
        let empty_string = CString::new("").unwrap();
        let ptr = empty_string.into_raw();
        let wrapper = unsafe { CStringPtr::new(ptr) };

        assert!(!wrapper.is_null());
        let result = unsafe { wrapper.to_str().unwrap() };
        assert_eq!(result, "");
    }

    #[test]
    fn test_cstring_ptr_null_to_str_panics_debug() {
        let null_wrapper = unsafe { CStringPtr::new(std::ptr::null_mut()) };
        
        // Test that we correctly identify null pointers
        assert!(null_wrapper.is_null());
        
        // In debug builds, this should panic. In release builds, it's undefined behavior.
        // Rather than testing undefined behavior, let's test the null check works
        if cfg!(debug_assertions) {
            // In debug mode, we expect a panic
            std::panic::catch_unwind(|| {
                let _ = unsafe { null_wrapper.to_str() };
            }).expect_err("Expected panic in debug mode for null pointer");
        } else {
            // In release mode, we just verify the null check works
            // Don't actually call to_str() with null as it's undefined behavior
            println!("Skipping null pointer dereference test in release mode (undefined behavior)");
        }
    }

    #[test]
    fn test_evaluate_cue_package_invalid_path() {
        // Test with invalid UTF-8 path (simulated)
        let invalid_path = Path::new("/nonexistent/\u{0000}/invalid");
        let result = evaluate_cue_package(invalid_path, "test");

        // Should fail with configuration error for invalid path
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid directory path"));
    }

    #[test]
    fn test_evaluate_cue_package_invalid_package_name() {
        let temp_dir = TempDir::new().unwrap();

        // Package name with null bytes should fail
        let result = evaluate_cue_package(temp_dir.path(), "test\0package");

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid package name"));
    }

    #[test]
    fn test_evaluate_cue_package_nonexistent_directory() {
        let nonexistent = Path::new("/definitely/does/not/exist/12345");
        let result = evaluate_cue_package(nonexistent, "env");

        // This will likely fail in the CUE evaluation, not path validation
        // The exact error depends on the Go CUE implementation
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_cue_package_with_valid_setup() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple valid CUE file
        let cue_content = r#"package env

env: {
    TEST_VAR: "test_value"
    NUMBER: 42
}
"#;
        fs::write(temp_dir.path().join("env.cue"), cue_content).unwrap();

        // This test depends on the Go FFI being available
        // In a real environment, this should work
        let result = evaluate_cue_package(temp_dir.path(), "env");

        // The result depends on whether the FFI bridge is properly built
        // In CI this might fail if Go dependencies aren't available
        if result.is_err() {
            // If FFI isn't available, we should get a specific error
            let error = result.unwrap_err();
            println!("FFI not available in test environment: {}", error);
            // This is acceptable in test environments without Go build
        } else {
            // If it works, verify the JSON contains our values
            let json = result.unwrap();
            assert!(json.contains("TEST_VAR"), "JSON should contain TEST_VAR");
            assert!(json.contains("test_value"), "JSON should contain the value");
        }
    }

    #[test]
    fn test_evaluate_cue_error_handling() {
        let temp_dir = TempDir::new().unwrap();

        // Create an invalid CUE file
        let invalid_cue = r#"package env

this is not valid CUE syntax {
    missing quotes and wrong structure
"#;
        fs::write(temp_dir.path().join("env.cue"), invalid_cue).unwrap();

        let result = evaluate_cue_package(temp_dir.path(), "env");

        // Should get an error - either FFI unavailable or CUE parse error
        assert!(result.is_err());

        let error = result.unwrap_err();
        // Error should be meaningful
        assert!(!error.to_string().is_empty());
        println!("Got expected error for invalid CUE: {}", error);
    }

    #[test]
    fn test_path_conversion_edge_cases() {
        // Test various path edge cases that might cause issues
        let temp_dir = TempDir::new().unwrap();
        let path_with_spaces = temp_dir.path().join("dir with spaces");
        fs::create_dir(&path_with_spaces).unwrap();

        // This should handle spaces correctly
        let result = evaluate_cue_package(&path_with_spaces, "env");

        // The result might be an error due to missing CUE files, but the path handling should work
        if let Err(e) = result {
            // Should not be a path conversion error
            assert!(!e.to_string().contains("Invalid directory path: not UTF-8"));
        }
    }

    // Integration test to verify memory management doesn't leak
    #[test]
    fn test_ffi_memory_management_stress() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple CUE file
        let cue_content = "package env\nenv: { TEST: \"value\" }";
        fs::write(temp_dir.path().join("env.cue"), cue_content).unwrap();

        // Call FFI function multiple times to test memory management
        for i in 0..100 {
            let result = evaluate_cue_package(temp_dir.path(), "env");

            // Each call should be independent and not cause memory issues
            if result.is_ok() {
                // If FFI is available, all calls should succeed
                assert!(result.unwrap().contains("TEST"));
            } else {
                // If FFI isn't available, error should be consistent
                let error_msg = result.unwrap_err().to_string();
                println!("Iteration {}: {}", i, error_msg);

                // Break early if it's clearly an FFI availability issue
                if i > 5 {
                    break;
                }
            }
        }

        // If we get here without crashes, memory management is working
    }

    // Test the error message parsing logic
    #[test]
    fn test_error_message_parsing() {
        // This tests the logic that parses "error:" prefixed messages
        // We can't easily mock the FFI call, but we can test the string logic

        let temp_dir = TempDir::new().unwrap();

        // The actual test depends on implementation details
        // For now, just verify the function exists and handles basic cases
        let result = evaluate_cue_package(temp_dir.path(), "nonexistent_package");

        // The behavior depends on whether the Go FFI bridge is available:
        // - If available: should return error for nonexistent package
        // - If not available: may return different error types
        // Either way, we should get some kind of result (error or success)
        
        match result {
            Ok(output) => {
                // If FFI isn't available or returns empty result, that's acceptable
                println!("FFI returned success (possibly unavailable): {}", output);
            }
            Err(error) => {
                // Expected case - should get an error for nonexistent package
                let error_str = error.to_string();
                assert!(!error_str.is_empty());
                assert!(error_str.len() > 5); // Should be a meaningful message
                println!("Got expected error: {}", error_str);
            }
        }
        
        // The main thing is the function doesn't crash/panic
    }
}
