use anyhow::{Context, Result};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

#[link(name = "cue_bridge")]
extern "C" {
    fn cue_parse_string(content: *const c_char) -> *mut c_char;
    fn cue_free_string(s: *mut c_char);
}

pub struct CueParser;

impl CueParser {
    pub fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read CUE file: {}", path.display()))?;
        
        Self::parse_content(&content)
    }

    pub fn parse_content(content: &str) -> Result<HashMap<String, String>> {
        let c_content = CString::new(content)
            .context("Failed to create C string from content")?;
        
        let result_ptr = unsafe { cue_parse_string(c_content.as_ptr()) };
        
        if result_ptr.is_null() {
            anyhow::bail!("Failed to parse CUE content");
        }
        
        let result_cstr = unsafe { CStr::from_ptr(result_ptr) };
        let result_str = result_cstr.to_str()
            .context("Failed to convert C string to Rust string")?;
        
        let env_vars = if result_str.is_empty() {
            HashMap::new()
        } else {
            // Parse JSON result
            let json_value: serde_json::Value = serde_json::from_str(result_str)
                .context("Failed to parse JSON result from CUE parser")?;
            
            // Check if it's an error response
            if let serde_json::Value::Object(ref map) = json_value {
                if let Some(serde_json::Value::String(error)) = map.get("error") {
                    anyhow::bail!("CUE parse error: {}", error);
                }
            }
            
            Self::extract_env_vars(&json_value)?
        };
        
        // Free the C string
        unsafe { cue_free_string(result_ptr) };
        
        Ok(env_vars)
    }

    fn extract_env_vars(value: &serde_json::Value) -> Result<HashMap<String, String>> {
        let mut env_vars = HashMap::new();
        
        if let serde_json::Value::Object(map) = value {
            for (key, val) in map {
                match val {
                    serde_json::Value::String(s) => {
                        env_vars.insert(key.clone(), s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        env_vars.insert(key.clone(), n.to_string());
                    }
                    serde_json::Value::Bool(b) => {
                        env_vars.insert(key.clone(), b.to_string());
                    }
                    _ => {
                        log::warn!("Skipping non-primitive value for key: {}", key);
                    }
                }
            }
        } else {
            anyhow::bail!("CUE file must contain an object at the root level");
        }
        
        Ok(env_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_env() {
        let content = r#"package env

DATABASE_URL: "postgres://localhost/mydb"
API_KEY: "secret123"
DEBUG: true
PORT: 3000"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.get("DATABASE_URL").unwrap(), "postgres://localhost/mydb");
        assert_eq!(result.get("API_KEY").unwrap(), "secret123");
        assert_eq!(result.get("DEBUG").unwrap(), "true");
        assert_eq!(result.get("PORT").unwrap(), "3000");
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"package env

// This is a comment
DATABASE_URL: "postgres://localhost/mydb"
// Multi-line comments in CUE use //
// not /* */
API_KEY: "secret123"
// Another comment
DEBUG: true"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.get("DATABASE_URL").unwrap(), "postgres://localhost/mydb");
        assert_eq!(result.get("API_KEY").unwrap(), "secret123");
        assert_eq!(result.get("DEBUG").unwrap(), "true");
    }

    #[test]
    fn test_parse_cue_features() {
        let content = r#"package env

// CUE supports string interpolation
BASE_URL: "https://api.example.com"
API_ENDPOINT: "\(BASE_URL)/v1"

// Default values
PORT: *3000 | int

// Constraints
TIMEOUT: >=0 & <=3600 & int | *30"#;

        let result = CueParser::parse_content(content).unwrap();
        // The CUE parser will evaluate these expressions
        assert!(result.contains_key("BASE_URL"));
        assert!(result.contains_key("PORT"));
    }

    #[test]
    fn test_package_requirement() {
        let content = r#"{
            DATABASE_URL: "postgres://localhost/mydb"
        }"#;

        let result = CueParser::parse_content(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("package env"));
    }
}