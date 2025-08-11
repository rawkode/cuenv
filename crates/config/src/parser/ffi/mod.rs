//! FFI bridge to CUE evaluation library
//!
//! This module handles the low-level interaction with the Go-based CUE library
//! through C FFI, including memory management and string conversion.

mod bridge;
mod memory;

pub use bridge::CueParser;

#[link(name = "cue_bridge")]
extern "C" {
    fn cue_eval_package(dir_path: *const std::os::raw::c_char, package_name: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn cue_free_string(s: *mut std::os::raw::c_char);
}