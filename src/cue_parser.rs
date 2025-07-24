use crate::errors::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

#[link(name = "cue_bridge")]
extern "C" {
    fn cue_parse_string(content: *const c_char) -> *mut c_char;
    fn cue_eval_package(dir_path: *const c_char, package_name: *const c_char) -> *mut c_char;
    fn cue_free_string(s: *mut c_char);
}

#[derive(Debug, Deserialize)]
struct CueParseResult {
    variables: HashMap<String, serde_json::Value>,
    metadata: HashMap<String, VariableMetadata>,
    environments: HashMap<String, HashMap<String, serde_json::Value>>,
    commands: HashMap<String, CommandConfig>,
    hooks: Option<HooksConfig>,
}

#[derive(Debug, Deserialize)]
struct HooksConfig {
    #[serde(rename = "onEnter")]
    on_enter: Option<HookConfig>,
    #[serde(rename = "onExit")]
    on_exit: Option<HookConfig>,
}

#[derive(Debug, Deserialize)]
struct VariableMetadata {
    capability: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandConfig {
    pub capabilities: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HookType {
    #[default]
    OnEnter,
    OnExit,
}

#[derive(Debug, Deserialize)]
pub struct HookConfig {
    pub command: String,
    pub args: Vec<String>,
    pub url: Option<String>,
    #[serde(skip)]
    pub hook_type: HookType,
}

pub struct CueParser;

impl CueParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CueParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct ParseOptions {
    pub environment: Option<String>,
    pub capabilities: Vec<String>,
}

pub struct ParseResult {
    pub variables: HashMap<String, String>,
    pub commands: HashMap<String, CommandConfig>,
    pub hooks: HashMap<String, HookConfig>,
}

impl CueParser {
    pub fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return Err(Error::file_system(path.to_path_buf(), "read CUE file", e)),
        };

        Self::parse_content(&content)
    }

    pub fn parse_env_file_with_options(path: &Path, options: &ParseOptions) -> Result<ParseResult> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return Err(Error::file_system(path.to_path_buf(), "read CUE file", e)),
        };

        Self::parse_content_with_options(&content, options)
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
        let c_dir = match CString::new(dir.to_string_lossy().as_ref()) {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::ffi(
                    "cue_eval_package",
                    format!("failed to create C string from directory path: {e}"),
                ));
            }
        };

        let c_package = match CString::new(package_name) {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::ffi(
                    "cue_eval_package",
                    format!("failed to create C string from package name: {e}"),
                ));
            }
        };

        let result_ptr = unsafe { cue_eval_package(c_dir.as_ptr(), c_package.as_ptr()) };

        if result_ptr.is_null() {
            return Err(Error::cue_parse(dir, "CUE parser returned null pointer"));
        }

        let result_cstr = unsafe { CStr::from_ptr(result_ptr) };
        let result_str = match result_cstr.to_str() {
            Ok(s) => s,
            Err(e) => {
                unsafe { cue_free_string(result_ptr) };
                return Err(Error::ffi(
                    "cue_eval_package",
                    format!("failed to convert C string to Rust string: {e}"),
                ));
            }
        };

        let parse_result = if result_str.is_empty() {
            ParseResult {
                variables: HashMap::new(),
                commands: HashMap::new(),
                hooks: HashMap::new(),
            }
        } else {
            // Parse JSON result
            let json_value: serde_json::Value = match serde_json::from_str(result_str) {
                Ok(v) => v,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::Json {
                        message: "failed to parse JSON result from CUE parser".to_string(),
                        source: e,
                    });
                }
            };

            // Check if it's an error response
            if let serde_json::Value::Object(ref map) = json_value {
                if let Some(serde_json::Value::String(error)) = map.get("error") {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::cue_parse(dir, error.clone()));
                }
            }

            // Deserialize into structured result
            let cue_result: CueParseResult = match serde_json::from_value(json_value) {
                Ok(r) => r,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::Json {
                        message: "failed to parse CUE result structure".to_string(),
                        source: e,
                    });
                }
            };

            match Self::build_parse_result(cue_result, options) {
                Ok(r) => r,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(e);
                }
            }
        };

        // Free the C string
        unsafe { cue_free_string(result_ptr) };

        Ok(parse_result)
    }

    pub fn parse_content(content: &str) -> Result<HashMap<String, String>> {
        match Self::parse_content_with_options(content, &ParseOptions::default()) {
            Ok(result) => Ok(result.variables),
            Err(e) => Err(e),
        }
    }

    pub fn parse_content_with_options(
        content: &str,
        options: &ParseOptions,
    ) -> Result<ParseResult> {
        let c_content = match CString::new(content) {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::ffi(
                    "cue_parse_string",
                    format!("failed to create C string from content: {e}"),
                ));
            }
        };

        let result_ptr = unsafe { cue_parse_string(c_content.as_ptr()) };

        if result_ptr.is_null() {
            return Err(Error::cue_parse(
                Path::new("<inline>"),
                "CUE parser returned null pointer",
            ));
        }

        let result_cstr = unsafe { CStr::from_ptr(result_ptr) };
        let result_str = match result_cstr.to_str() {
            Ok(s) => s,
            Err(e) => {
                unsafe { cue_free_string(result_ptr) };
                return Err(Error::ffi(
                    "cue_parse_string",
                    format!("failed to convert C string to Rust string: {e}"),
                ));
            }
        };

        let parse_result = if result_str.is_empty() {
            ParseResult {
                variables: HashMap::new(),
                commands: HashMap::new(),
                hooks: HashMap::new(),
            }
        } else {
            // Parse JSON result
            let json_value: serde_json::Value = match serde_json::from_str(result_str) {
                Ok(v) => v,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::Json {
                        message: "failed to parse JSON result from CUE parser".to_string(),
                        source: e,
                    });
                }
            };

            // Check if it's an error response
            if let serde_json::Value::Object(ref map) = json_value {
                if let Some(serde_json::Value::String(error)) = map.get("error") {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::cue_parse(Path::new("<inline>"), error.clone()));
                }
            }

            // Deserialize into structured result
            let cue_result: CueParseResult = match serde_json::from_value(json_value) {
                Ok(r) => r,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(Error::Json {
                        message: "failed to parse CUE result structure".to_string(),
                        source: e,
                    });
                }
            };

            match Self::build_parse_result(cue_result, options) {
                Ok(r) => r,
                Err(e) => {
                    unsafe { cue_free_string(result_ptr) };
                    return Err(e);
                }
            }
        };

        // Free the C string
        unsafe { cue_free_string(result_ptr) };

        Ok(parse_result)
    }

    fn build_parse_result(
        cue_result: CueParseResult,
        options: &ParseOptions,
    ) -> Result<ParseResult> {
        let mut final_vars = HashMap::new();

        // Start with base variables
        for (key, val) in &cue_result.variables {
            // Check capability filter
            let should_include = if !options.capabilities.is_empty() {
                if let Some(metadata) = cue_result.metadata.get(key) {
                    if let Some(cap) = &metadata.capability {
                        options.capabilities.contains(cap)
                    } else {
                        false // No capability requirement when filter is active, exclude it
                    }
                } else {
                    false // No metadata when filter is active, exclude it
                }
            } else {
                true // No capability filter, include everything
            };

            if should_include {
                if let Some(str_val) = Self::value_to_string(val) {
                    final_vars.insert(key.clone(), str_val);
                }
            }
        }

        // Apply environment-specific overrides
        if let Some(env_name) = &options.environment {
            if let Some(env_vars) = cue_result.environments.get(env_name) {
                for (key, val) in env_vars {
                    // For environment overrides, we still need to check capabilities
                    // if they were specified in the base variable
                    let should_include = if !options.capabilities.is_empty() {
                        if let Some(metadata) = cue_result.metadata.get(key) {
                            if let Some(cap) = &metadata.capability {
                                options.capabilities.contains(cap)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        true
                    };

                    if should_include {
                        if let Some(str_val) = Self::value_to_string(val) {
                            final_vars.insert(key.clone(), str_val);
                        }
                    }
                }
            }
        }

        // Process hooks
        let mut hooks = HashMap::new();
        if let Some(hooks_config) = cue_result.hooks {
            if let Some(mut on_enter) = hooks_config.on_enter {
                on_enter.hook_type = HookType::OnEnter;
                hooks.insert("onEnter".to_string(), on_enter);
            }
            if let Some(mut on_exit) = hooks_config.on_exit {
                on_exit.hook_type = HookType::OnExit;
                hooks.insert("onExit".to_string(), on_exit);
            }
        }

        Ok(ParseResult {
            variables: final_vars,
            commands: cue_result.commands,
            hooks,
        })
    }

    fn value_to_string(val: &serde_json::Value) -> Option<String> {
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

    #[allow(dead_code)]
    fn extract_env_vars(value: &serde_json::Value) -> Result<HashMap<String, String>> {
        let mut env_vars = HashMap::new();

        if let serde_json::Value::Object(map) = value {
            // Handle both old format (direct variables) and new format (with "variables" key)
            let vars_map = if let Some(serde_json::Value::Object(m)) = map.get("variables") {
                m
            } else {
                map
            };

            for (key, val) in vars_map {
                if let Some(str_val) = Self::value_to_string(val) {
                    env_vars.insert(key.clone(), str_val);
                }
            }
        } else {
            return Err(Error::cue_parse(
                Path::new("<inline>"),
                "CUE file must contain an object at the root level",
            ));
        }

        Ok(env_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_env() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
    DEBUG: true
    PORT: 3000
}"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(
            result.get("DATABASE_URL").unwrap(),
            "postgres://localhost/mydb"
        );
        assert_eq!(result.get("API_KEY").unwrap(), "secret123");
        assert_eq!(result.get("DEBUG").unwrap(), "true");
        assert_eq!(result.get("PORT").unwrap(), "3000");
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"package cuenv

env: {
    // This is a comment
    DATABASE_URL: "postgres://localhost/mydb"
    // Multi-line comments in CUE use //
    // not /* */
    API_KEY: "secret123"
    // Another comment
    DEBUG: true
}"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(
            result.get("DATABASE_URL").unwrap(),
            "postgres://localhost/mydb"
        );
        assert_eq!(result.get("API_KEY").unwrap(), "secret123");
        assert_eq!(result.get("DEBUG").unwrap(), "true");
    }

    #[test]
    fn test_parse_cue_features() {
        let content = r#"package cuenv

env: {
    // CUE supports string interpolation
    BASE_URL: "https://api.example.com"
    API_ENDPOINT: "\(BASE_URL)/v1"
    
    // Default values
    PORT: *3000 | int
    
    // Constraints
    TIMEOUT: >=0 & <=3600 & int | *30
}"#;

        let result = CueParser::parse_content(content).unwrap();
        // The CUE parser will evaluate these expressions
        assert!(result.contains_key("BASE_URL"));
        assert!(result.contains_key("PORT"));
    }

    #[test]
    fn test_package_requirement() {
        let content = r#"{
            env: {
                DATABASE_URL: "postgres://localhost/mydb"
            }
        }"#;

        let result = CueParser::parse_content(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("package cuenv"));
    }

    #[test]
    fn test_parse_with_environments() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
    PORT: 3000
    
    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
            PORT: 8080
        }
        staging: {
            DATABASE_URL: "postgres://staging.example.com/mydb"
            API_KEY: "staging-key"
        }
    }
}"#;

        // Test default parsing (no environment)
        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(
            result.get("DATABASE_URL").unwrap(),
            "postgres://localhost/mydb"
        );
        assert_eq!(result.get("API_KEY").unwrap(), "secret123");
        assert_eq!(result.get("PORT").unwrap(), "3000");

        // Test with production environment
        let options = ParseOptions {
            environment: Some("production".to_string()),
            capabilities: Vec::new(),
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(
            result.variables.get("DATABASE_URL").unwrap(),
            "postgres://prod.example.com/mydb"
        );
        assert_eq!(result.variables.get("API_KEY").unwrap(), "secret123"); // Not overridden
        assert_eq!(result.variables.get("PORT").unwrap(), "8080");

        // Test with staging environment
        let options = ParseOptions {
            environment: Some("staging".to_string()),
            capabilities: Vec::new(),
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(
            result.variables.get("DATABASE_URL").unwrap(),
            "postgres://staging.example.com/mydb"
        );
        assert_eq!(result.variables.get("API_KEY").unwrap(), "staging-key");
        assert_eq!(result.variables.get("PORT").unwrap(), "3000"); // Not overridden
    }

    #[test]
    fn test_parse_with_capabilities() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
    AWS_ACCESS_KEY: "aws-key" @capability("aws")
    AWS_SECRET_KEY: "aws-secret" @capability("aws")
}"#;

        // Test without capability filter
        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.len(), 4);
        assert!(result.contains_key("DATABASE_URL"));
        assert!(result.contains_key("API_KEY"));
        assert!(result.contains_key("AWS_ACCESS_KEY"));
        assert!(result.contains_key("AWS_SECRET_KEY"));

        // Test with aws capability filter
        let options = ParseOptions {
            environment: None,
            capabilities: vec!["aws".to_string()],
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(result.variables.len(), 2);
        assert!(result.variables.contains_key("AWS_ACCESS_KEY"));
        assert!(result.variables.contains_key("AWS_SECRET_KEY"));
        assert!(!result.variables.contains_key("DATABASE_URL"));
        assert!(!result.variables.contains_key("API_KEY"));

        // Test with non-existent capability
        let options = ParseOptions {
            environment: None,
            capabilities: vec!["gcp".to_string()],
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(result.variables.len(), 0);
    }

    #[test]
    fn test_parse_with_commands() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    
    Commands: {
        migrate: {
            capabilities: ["database"]
        }
        deploy: {
            capabilities: ["aws", "docker"]
        }
        test: {}
    }
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert!(result.commands.contains_key("migrate"));
        assert!(result.commands.contains_key("deploy"));
        assert!(result.commands.contains_key("test"));

        let migrate_cmd = &result.commands["migrate"];
        assert_eq!(
            migrate_cmd.capabilities.as_ref().unwrap(),
            &vec!["database".to_string()]
        );

        let deploy_cmd = &result.commands["deploy"];
        assert_eq!(
            deploy_cmd.capabilities.as_ref().unwrap(),
            &vec!["aws".to_string(), "docker".to_string()]
        );

        let test_cmd = &result.commands["test"];
        assert!(test_cmd.capabilities.is_none());
    }

    #[test]
    fn test_parse_with_env_and_capabilities() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    API_KEY: "secret123"
    AWS_ACCESS_KEY: "aws-key-dev" @capability("aws")
    
    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
            AWS_ACCESS_KEY: "aws-key-prod" @capability("aws")
        }
    }
}"#;

        // Test production environment with aws capability
        let options = ParseOptions {
            environment: Some("production".to_string()),
            capabilities: vec!["aws".to_string()],
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables.get("AWS_ACCESS_KEY").unwrap(),
            "aws-key-prod"
        );
        assert!(!result.variables.contains_key("DATABASE_URL")); // Filtered out by capability
        assert!(!result.variables.contains_key("API_KEY")); // Filtered out by capability
    }

    #[test]
    fn test_empty_cue_file() {
        let content = r#"package cuenv

env: {}"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_structured_secrets() {
        // Test with simpler CUE syntax that the parser can handle
        let content = r#"package cuenv

env: {
    // Regular variables
    DATABASE_URL: "postgres://localhost/mydb"
    
    // Secret references in string format
    AWS_KEY: "op://Personal/aws/key"
    DB_PASS: "op://Work/database/password"
    
    // Traditional secret format
    STRIPE_KEY: "op://Work/stripe/key"
    GCP_SECRET: "gcp-secret://my-project/api-key"
}"#;

        let result = CueParser::parse_content(content).unwrap();

        // Regular variable
        assert_eq!(
            result.get("DATABASE_URL").unwrap(),
            "postgres://localhost/mydb"
        );

        // Secret references
        assert_eq!(result.get("AWS_KEY").unwrap(), "op://Personal/aws/key");
        assert_eq!(
            result.get("DB_PASS").unwrap(),
            "op://Work/database/password"
        );

        // Traditional secret references
        assert_eq!(result.get("STRIPE_KEY").unwrap(), "op://Work/stripe/key");
        assert_eq!(
            result.get("GCP_SECRET").unwrap(),
            "gcp-secret://my-project/api-key"
        );
    }

    #[test]
    fn test_parse_with_nested_objects() {
        let content = r#"package cuenv

env: {
    DATABASE: {
        host: "localhost"
        port: 5432
    }
}"#;

        // The parser should skip non-primitive values
        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_value_types() {
        let content = r#"package cuenv

env: {
    STRING_VAL: "hello"
    INT_VAL: 42
    FLOAT_VAL: 3.14
    BOOL_VAL: true
    NULL_VAL: null
    ARRAY_VAL: [1, 2, 3]
    OBJECT_VAL: {nested: "value"}
}"#;

        let result = CueParser::parse_content(content).unwrap();
        assert_eq!(result.get("STRING_VAL").unwrap(), "hello");
        assert_eq!(result.get("INT_VAL").unwrap(), "42");
        assert_eq!(result.get("FLOAT_VAL").unwrap(), "3.14");
        assert_eq!(result.get("BOOL_VAL").unwrap(), "true");
        // null, arrays, and objects should be skipped
        assert!(!result.contains_key("NULL_VAL"));
        assert!(!result.contains_key("ARRAY_VAL"));
        assert!(!result.contains_key("OBJECT_VAL"));
    }

    #[test]
    fn test_parse_with_hooks() {
        let content = r#"package cuenv

hooks: {
    onEnter: {
        command: "echo"
        args: ["Entering environment"]
    }
    onExit: {
        command: "cleanup.sh"
        args: ["--verbose"]
    }
}

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 2);

        let on_enter = result.hooks.get("onEnter").unwrap();
        assert_eq!(on_enter.command, "echo");
        assert_eq!(on_enter.args, vec!["Entering environment"]);
        assert_eq!(on_enter.hook_type, HookType::OnEnter);
        assert!(on_enter.url.is_none());

        let on_exit = result.hooks.get("onExit").unwrap();
        assert_eq!(on_exit.command, "cleanup.sh");
        assert_eq!(on_exit.args, vec!["--verbose"]);
        assert_eq!(on_exit.hook_type, HookType::OnExit);
        assert!(on_exit.url.is_none());
    }

    #[test]
    fn test_parse_hooks_with_url() {
        let content = r#"package cuenv

hooks: {
    onEnter: {
        command: "notify"
        args: ["webhook", "start"]
        url: "https://example.com/webhook"
    }
}

env: {
    API_KEY: "secret123"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 1);

        let hook = result.hooks.get("onEnter").unwrap();
        assert_eq!(hook.command, "notify");
        assert_eq!(hook.args, vec!["webhook", "start"]);
        assert_eq!(hook.url, Some("https://example.com/webhook".to_string()));
    }

    #[test]
    fn test_parse_empty_hooks() {
        let content = r#"package cuenv

hooks: {}

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 0);
    }

    #[test]
    fn test_parse_no_hooks() {
        let content = r#"package cuenv

env: {
    DATABASE_URL: "postgres://localhost/mydb"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 0);
    }

    #[test]
    fn test_parse_hooks_with_complex_args() {
        let content = r#"package cuenv

hooks: {
    onEnter: {
        command: "docker"
        args: ["run", "-d", "--name", "test-db", "postgres:14"]
    }
    onExit: {
        command: "docker"
        args: ["stop", "test-db", "&&", "docker", "rm", "test-db"]
    }
}

env: {
    APP_NAME: "myapp"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        let on_enter = result.hooks.get("onEnter").unwrap();
        assert_eq!(on_enter.args.len(), 5);
        assert_eq!(on_enter.args[0], "run");
        assert_eq!(on_enter.args[4], "postgres:14");

        let on_exit = result.hooks.get("onExit").unwrap();
        assert_eq!(on_exit.args.len(), 6);
    }

    #[test]
    fn test_parse_hooks_with_environments() {
        let content = r#"package cuenv

hooks: {
    onEnter: {
        command: "echo"
        args: ["Development environment"]
    }
}

env: {
    DATABASE_URL: "postgres://localhost/mydb"
    
    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
        }
    }
}"#;

        // Test with development (default)
        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(result.hooks.len(), 1);
        assert_eq!(
            result.hooks.get("onEnter").unwrap().args[0],
            "Development environment"
        );

        // Test with production environment - hooks should remain the same
        let options = ParseOptions {
            environment: Some("production".to_string()),
            capabilities: Vec::new(),
        };
        let result = CueParser::parse_content_with_options(content, &options).unwrap();
        assert_eq!(result.hooks.len(), 1);
        assert_eq!(
            result.hooks.get("onEnter").unwrap().args[0],
            "Development environment"
        );
    }

    #[test]
    fn test_parse_hooks_only_on_enter() {
        let content = r#"package cuenv

hooks: {
    onEnter: {
        command: "start-server"
        args: []
    }
}

env: {
    API_URL: "http://localhost:3000"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 1);
        assert!(result.hooks.contains_key("onEnter"));
        assert!(!result.hooks.contains_key("onExit"));

        let hook = result.hooks.get("onEnter").unwrap();
        assert_eq!(hook.command, "start-server");
        assert!(hook.args.is_empty());
    }

    #[test]
    fn test_parse_hooks_only_on_exit() {
        let content = r#"package cuenv

hooks: {
    onExit: {
        command: "stop-server"
        args: ["--graceful"]
    }
}

env: {
    API_URL: "http://localhost:3000"
}"#;

        let options = ParseOptions::default();
        let result = CueParser::parse_content_with_options(content, &options).unwrap();

        assert_eq!(result.hooks.len(), 1);
        assert!(!result.hooks.contains_key("onEnter"));
        assert!(result.hooks.contains_key("onExit"));

        let hook = result.hooks.get("onExit").unwrap();
        assert_eq!(hook.command, "stop-server");
        assert_eq!(hook.args, vec!["--graceful"]);
    }
}
