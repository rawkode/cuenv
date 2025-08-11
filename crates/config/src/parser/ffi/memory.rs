//! Memory management for FFI strings
//!
//! Provides RAII wrapper for C strings returned from FFI to ensure proper cleanup.

use cuenv_core::errors::{Error, Result};
use std::ffi::CStr;
use std::os::raw::c_char;

/// RAII wrapper for C strings returned from FFI
/// Ensures proper cleanup when the wrapper goes out of scope
pub(super) struct CStringPtr {
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
            // Safety: We only call cue_free_string on non-null pointers that were
            // returned from cue_eval_package. The FFI contract guarantees that
            // this is safe to call exactly once per returned pointer.
            unsafe {
                super::cue_free_string(self.ptr);
            }
        }
    }
}
