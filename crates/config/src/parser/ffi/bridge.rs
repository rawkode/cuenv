//! CUE parser bridge implementation
//!
//! Provides the main interface for evaluating CUE packages through FFI.

use super::memory::CStringPtr;
use crate::parser::processing::{build_parse_result, ParseOptions, ParseResult};
use crate::parser::types::CueParseResult;
use crate::parser::validation::{
    create_ffi_string, validate_directory_path, validate_package_name,
};
use cuenv_core::errors::{Error, Result};
use cuenv_utils::resilience::suggest_recovery;
use std::collections::HashMap;
use std::ffi::CStr;
use std::path::Path;

pub struct CueParser;

impl CueParser {
    pub fn new() -> Self {
        Self
    }

    pub fn eval_package(dir: &Path, package_name: &str) -> Result<HashMap<String, String>> {
        match Self::eval_package_with_options(dir, package_name, &ParseOptions::default()) {
            Ok(result) => Ok(result.variables),
            Err(e) => Err(e),
        }
    }

    pub fn eval_package_with_options(
        dir: &Path,
        package_name: &str,
        options: &ParseOptions,
    ) -> Result<ParseResult> {
        // Validate inputs
        validate_package_name(package_name)?;
        let dir_str = validate_directory_path(dir)?;

        // Create FFI strings
        let c_dir = create_ffi_string(&dir_str, "invalid directory path")?;
        let c_package = create_ffi_string(package_name, "invalid package name")?;

        // Call CUE evaluation
        let result_ptr = call_cue_eval_package(&c_dir, &c_package);

        // Wrap the result pointer for automatic cleanup
        // Safety: result_ptr is either null or a valid pointer returned from cue_eval_package
        let result_wrapper = unsafe { CStringPtr::new(result_ptr) };

        if result_wrapper.is_null() {
            return Err(Error::cue_parse(dir, "CUE parser returned null pointer"));
        }

        // Safety: We've verified the pointer is not null
        let result_str = unsafe { result_wrapper.to_str()? };

        let parse_result = if result_str.is_empty() {
            ParseResult::default()
        } else {
            // Parse and validate JSON response
            let json_value = parse_json_response(result_str)?;
            check_for_error_response(&json_value, dir)?;

            // Deserialize and build final result
            let cue_result = deserialize_cue_result(json_value)?;
            build_parse_result(cue_result, options)?
        };

        // The CStringPtr will be automatically freed when it goes out of scope
        Ok(parse_result)
    }

    pub fn value_to_string(val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            serde_json::Value::Null => None,
            _ => {
                log::warn!("Skipping non-primitive value");
                None
            }
        }
    }
}

impl Default for CueParser {
    fn default() -> Self {
        Self::new()
    }
}

fn call_cue_eval_package(dir_path: &CStr, package_name: &CStr) -> *mut std::os::raw::c_char {
    // Safety: cue_eval_package is an external C function that:
    // - Takes two non-null C string pointers as arguments
    // - Returns a heap-allocated C string that must be freed with cue_free_string
    // - Returns null on allocation failure
    // We ensure the input pointers are valid for the duration of the call
    unsafe { super::cue_eval_package(dir_path.as_ptr(), package_name.as_ptr()) }
}

fn parse_json_response(json_str: &str) -> Result<serde_json::Value> {
    serde_json::from_str(json_str).map_err(|e| {
        let error = Error::Json {
            message: "failed to parse JSON result from CUE parser".to_string(),
            source: e,
        };
        log::error!("CUE parser returned invalid JSON: {json_str}");
        log::error!("Recovery suggestion: {}", suggest_recovery(&error));
        error
    })
}

fn check_for_error_response(json_value: &serde_json::Value, dir: &Path) -> Result<()> {
    if let serde_json::Value::Object(ref map) = json_value {
        if let Some(serde_json::Value::String(error)) = map.get("error") {
            let cue_error = Error::cue_parse(dir, error.clone());

            // Provide specific recovery suggestions based on error content
            let recovery_hint = get_recovery_hint(error);

            log::error!("CUE parsing error: {error}");
            log::error!("Recovery suggestion: {recovery_hint}");
            return Err(cue_error);
        }
    }
    Ok(())
}

fn get_recovery_hint(error: &str) -> &'static str {
    if error.contains("cannot find package") {
        "Ensure your .cue files have 'package env' at the top"
    } else if error.contains("expected") || error.contains("syntax") {
        "Check for missing commas, brackets, or quotes in your CUE file"
    } else if error.contains("cycle") {
        "You have a circular dependency in your CUE definitions"
    } else if error.contains("incomplete") {
        "Some required fields are missing in your CUE configuration"
    } else {
        "Run 'cue vet' on your files to check for syntax errors"
    }
}

fn deserialize_cue_result(json_value: serde_json::Value) -> Result<CueParseResult> {
    serde_json::from_value(json_value).map_err(|e| {
        let error = Error::Json {
            message: "failed to parse CUE result structure".to_string(),
            source: e,
        };
        log::error!("Failed to deserialize CUE result. This might indicate a version mismatch.");
        log::error!("Recovery suggestion: {}", suggest_recovery(&error));
        error
    })
}
