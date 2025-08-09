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
            format!("CUE evaluation error: {}", error_msg),
        ));
    }

    Ok(json_str.to_string())
}
