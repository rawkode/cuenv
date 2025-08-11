//! Tests for the CUE parser module

use super::*;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

fn create_test_env(content: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let cue_dir = temp_dir.path().join("cue.mod");
    fs::create_dir(&cue_dir).unwrap();
    fs::write(cue_dir.join("module.cue"), "module: \"test.com/env\"").unwrap();

    let env_file = temp_dir.path().join("env.cue");
    fs::write(&env_file, content).unwrap();

    temp_dir
}

#[test]
#[serial]
fn test_only_env_package_allowed() {
    // Test that non-env packages are rejected
    let content = r#"
    package mypackage

    env: {
        DATABASE_URL: "postgresql://localhost/mydb"
    }"#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "mypackage");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Only 'env' package is supported"),
        "Error message was: {err_msg}"
    );

    // Test that env package is accepted
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgresql://localhost/mydb"
    }"#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env");
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_parse_simple_env() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
        API_KEY:      "secret123"
        DEBUG:        true
        PORT:         3000
    }
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();

    assert_eq!(
        result.get("DATABASE_URL").unwrap(),
        "postgres://localhost/mydb"
    );
    assert_eq!(result.get("API_KEY").unwrap(), "secret123");
    assert_eq!(result.get("DEBUG").unwrap(), "true");
    assert_eq!(result.get("PORT").unwrap(), "3000");
}

#[test]
#[serial]
fn test_parse_with_comments() {
    let content = r#"
    package env

    env: {
        // This is a comment
        DATABASE_URL: "postgres://localhost/mydb"
        // Multi-line comments in CUE use //
        // not /* */
        API_KEY: "secret123"
        // Another comment
        DEBUG: true
    }
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
    assert_eq!(
        result.get("DATABASE_URL").unwrap(),
        "postgres://localhost/mydb"
    );
    assert_eq!(result.get("API_KEY").unwrap(), "secret123");
    assert_eq!(result.get("DEBUG").unwrap(), "true");
}

#[test]
#[serial]
fn test_parse_cue_features() {
    let content = r#"
    package env

    env: {
        // CUE supports string interpolation
        BASE_URL: "https://api.example.com"
        API_ENDPOINT: "\(BASE_URL)/v1"

        // Default values
        PORT: *3000 | int

        // Constraints
        TIMEOUT: >=0 & <=3600 & int | *30
    }
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
    // The CUE parser will evaluate these expressions
    assert!(result.contains_key("BASE_URL"));
    assert!(result.contains_key("PORT"));
}

#[test]
#[serial]
fn test_package_requirement() {
    let content = r#"{
        env: {
            DATABASE_URL: "postgres://localhost/mydb"
        }
    }"#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env");
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_parse_with_environments() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
        API_KEY:      "secret123"
        PORT:         3000

        environment: {
            production: {
                DATABASE_URL: "postgres://prod.example.com/mydb"
                PORT:         8080
            }
            staging: {
                DATABASE_URL: "postgres://staging.example.com/mydb"
                API_KEY:      "staging-key"
            }
        }
    }
    "#;
    let temp_dir = create_test_env(content);

    // Test default parsing (no environment)
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
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
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
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
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(
        result.variables.get("DATABASE_URL").unwrap(),
        "postgres://staging.example.com/mydb"
    );
    assert_eq!(result.variables.get("API_KEY").unwrap(), "staging-key");
    assert_eq!(result.variables.get("PORT").unwrap(), "3000"); // Not overridden
}

#[test]
#[serial]
fn test_parse_with_capabilities() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
        API_KEY:      "secret123"
    }

    metadata: {
        AWS_ACCESS_KEY: capability: "aws"
        AWS_SECRET_KEY: capability: "aws"
    }
    "#;
    let temp_dir = create_test_env(content);

    // Test without capability filter
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("DATABASE_URL"));
    assert!(result.contains_key("API_KEY"));

    // Test with aws capability filter
    let options = ParseOptions {
        environment: None,
        capabilities: vec!["aws".to_string()],
    };
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(result.variables.len(), 2); // DATABASE_URL and API_KEY have no capabilities, so they're always included

    // Test with non-existent capability
    let options = ParseOptions {
        environment: None,
        capabilities: vec!["gcp".to_string()],
    };
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(result.variables.len(), 2); // DATABASE_URL and API_KEY have no capabilities, so they're always included
}

#[test]
#[serial]
fn test_parse_with_commands() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
    }

    capabilities: {
        database: {
            commands: ["migrate"]
        }
        aws: {
            commands: ["deploy"]
        }
        docker: {
            commands: ["deploy", "test"]
        }
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert!(result.commands.contains_key("migrate"));
    assert!(result.commands.contains_key("deploy"));
    assert!(result.commands.contains_key("test"));

    let migrate_cmd = &result.commands["migrate"];
    assert_eq!(
        migrate_cmd.capabilities.as_ref().unwrap(),
        &vec!["database".to_string()]
    );

    let deploy_cmd = &result.commands["deploy"];
    let mut expected_caps = vec!["aws".to_string(), "docker".to_string()];
    let mut actual_caps = deploy_cmd.capabilities.as_ref().unwrap().clone();
    expected_caps.sort();
    actual_caps.sort();
    assert_eq!(actual_caps, expected_caps);

    let test_cmd = &result.commands["test"];
    assert_eq!(
        test_cmd.capabilities.as_ref().unwrap(),
        &vec!["docker".to_string()]
    );
}

#[test]
#[serial]
fn test_parse_with_env_and_capabilities() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
        API_KEY:      "secret123"
        AWS_ACCESS_KEY: "aws-key-dev"

        environment: {
            production: {
                DATABASE_URL: "postgres://prod.example.com/mydb"
                AWS_ACCESS_KEY: "aws-key-prod"
            }
        }
    }

    metadata: {
        AWS_ACCESS_KEY: capability: "aws"
    }
    "#;
    let temp_dir = create_test_env(content);

    // Test production environment with aws capability
    let options = ParseOptions {
        environment: Some("production".to_string()),
        capabilities: vec!["aws".to_string()],
    };
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(result.variables.len(), 3);
    assert_eq!(
        result.variables.get("AWS_ACCESS_KEY").unwrap(),
        "aws-key-prod"
    );
    assert_eq!(
        result.variables.get("DATABASE_URL").unwrap(),
        "postgres://prod.example.com/mydb"
    );
    assert_eq!(result.variables.get("API_KEY").unwrap(), "secret123")
}

#[test]
#[serial]
fn test_empty_cue_file() {
    let content = r#"
    package env

    env: {}
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
#[serial]
fn test_structured_secrets() {
    // Test with simpler CUE syntax that the parser can handle
    let content = r#"
    package env

    env: {
        // Regular variables
        DATABASE_URL: "postgres://localhost/mydb"

        // Secret references in string format
        AWS_KEY: "op://Personal/aws/key"
        DB_PASS: "op://Work/database/password"

        // Traditional secret format
        STRIPE_KEY: "op://Work/stripe/key"
        GCP_SECRET: "gcp-secret://my-project/api-key"
    }
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();

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
#[serial]
fn test_parse_with_nested_objects() {
    let content = r#"
    package env

    env: {
        DATABASE: {
            host: "localhost"
            port: 5432
        }
    }
    "#;
    let temp_dir = create_test_env(content);
    // The parser should skip non-primitive values
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
#[serial]
fn test_value_types() {
    let content = r#"
    package env

    env: {
        STRING_VAL: "hello"
        INT_VAL:    42
        FLOAT_VAL:  3.14
        BOOL_VAL:   true
        NULL_VAL:   null
        ARRAY_VAL: [1, 2, 3]
        OBJECT_VAL: {nested: "value"}
    }
    "#;
    let temp_dir = create_test_env(content);
    let result = CueParser::eval_package(temp_dir.path(), "env").unwrap();
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
#[serial]
fn test_parse_with_hooks() {
    let content = r#"
    package env

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
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 2);

    let on_enter = &result.hooks.get("onEnter").unwrap()[0];
    match on_enter {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "echo");
            assert_eq!(hook_config.args, vec!["Entering environment"]);
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "echo");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["Entering environment"]);
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    let on_exit = &result.hooks.get("onExit").unwrap()[0];
    match on_exit {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "cleanup.sh");
            assert_eq!(hook_config.args, vec!["--verbose"]);
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "cleanup.sh");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["--verbose"]);
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_with_url() {
    let content = r#"
    package env

    hooks: {
        onEnter: {
            command: "notify"
            args:    ["webhook", "start"]
            url:     "https://example.com/webhook"
        }
    }

    env: {
        API_KEY: "secret123"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 1);

    let hook = &result.hooks.get("onEnter").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "notify");
            assert_eq!(hook_config.args, vec!["webhook", "start"]);
            assert_eq!(
                hook_config.url,
                Some("https://example.com/webhook".to_string())
            );
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "notify");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["webhook", "start"]);
            // TODO: URL support needs to be added to ExecConfig if needed
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_empty_hooks() {
    let content = r#"
    package env

    hooks: {}

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 0);
}

#[test]
#[serial]
fn test_parse_no_hooks() {
    let content = r#"
    package env

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 0);
}

#[test]
#[serial]
fn test_parse_hooks_with_complex_args() {
    let content = r#"
    package env

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
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    let on_enter = &result.hooks.get("onEnter").unwrap()[0];
    match on_enter {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.args.len(), 5);
            assert_eq!(hook_config.args[0], "run");
            assert_eq!(hook_config.args[4], "postgres:14");
        }
        Hook::Exec { exec, .. } => {
            let args = exec.args.as_ref().unwrap();
            assert_eq!(args.len(), 5);
            assert_eq!(args[0], "run");
            assert_eq!(args[4], "postgres:14");
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    let on_exit = &result.hooks.get("onExit").unwrap()[0];
    match on_exit {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.args.len(), 6);
        }
        Hook::Exec { exec, .. } => {
            let args = exec.args.as_ref().unwrap();
            assert_eq!(args.len(), 6);
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_with_environments() {
    let content = r#"
    package env

    hooks: {
        onEnter: {
            command: "echo"
            args: ["Development environment"]
        }
    }

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
    }

    environment: {
        production: {
            DATABASE_URL: "postgres://prod.example.com/mydb"
        }
    }
    "#;
    let temp_dir = create_test_env(content);

    // Test with development (default)
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(result.hooks.len(), 1);
    let hook = &result.hooks.get("onEnter").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.args[0], "Development environment");
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.args.as_ref().unwrap()[0], "Development environment");
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    // Test with production environment - hooks should remain the same
    let options = ParseOptions {
        environment: Some("production".to_string()),
        capabilities: Vec::new(),
    };
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();
    assert_eq!(result.hooks.len(), 1);
    let hook = &result.hooks.get("onEnter").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.args[0], "Development environment");
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.args.as_ref().unwrap()[0], "Development environment");
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_only_on_enter() {
    let content = r#"
    package env

    hooks: {
        onEnter: {
            command: "start-server"
            args: []
        }
    }

    env: {
        API_URL: "http://localhost:3000"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 1);
    assert!(result.hooks.contains_key("onEnter"));
    assert!(!result.hooks.contains_key("onExit"));

    let hook = &result.hooks.get("onEnter").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "start-server");
            assert!(hook_config.args.is_empty());
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "start-server");
            assert!(exec.args.as_ref().is_none_or(|args| args.is_empty()));
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_only_on_exit() {
    let content = r#"
    package env

    hooks: {
        onExit: {
            command: "stop-server"
            args: ["--graceful"]
        }
    }

    env: {
        API_URL: "http://localhost:3000"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 1);
    assert!(!result.hooks.contains_key("onEnter"));
    assert!(result.hooks.contains_key("onExit"));

    let hook = &result.hooks.get("onExit").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "stop-server");
            assert_eq!(hook_config.args, vec!["--graceful"]);
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "stop-server");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["--graceful"]);
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_with_constraints() {
    let content = r#"
    package env

    hooks: {
        onEnter: {
            command: "devenv"
            args: ["up"]
            constraints: [
                {
                    commandExists: {
                        command: "devenv"
                    }
                },
                {
                    shellCommand: {
                        command: "nix"
                        args: ["--version"]
                    }
                }
            ]
        }
        onExit: {
            command: "cleanup.sh"
            args: []
            constraints: [
                {
                    shellCommand: {
                        command: "test"
                        args: ["-f", "/tmp/cleanup_needed"]
                    }
                },
                {
                    commandExists: {
                        command: "cleanup"
                    }
                }
            ]
        }
    }

    env: {
        DATABASE_URL: "postgres://localhost/mydb"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 2);

    // Test onEnter hook constraints
    let on_enter = &result.hooks.get("onEnter").unwrap()[0];
    match on_enter {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "devenv");
            assert_eq!(hook_config.args, vec!["up"]);
            assert_eq!(hook_config.constraints.len(), 2);
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "devenv");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["up"]);
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    // Check constraints (available in both Legacy and Exec formats)
    match on_enter {
        Hook::Legacy(hook_config) => {
            // Check first constraint - command exists
            if let HookConstraint::CommandExists { command } = &hook_config.constraints[0] {
                assert_eq!(command, "devenv");
            } else {
                panic!("Expected CommandExists constraint");
            }

            // Check second constraint - shell command
            if let HookConstraint::ShellCommand { command, args } = &hook_config.constraints[1]
            {
                assert_eq!(command, "nix");
                assert_eq!(args.as_ref().unwrap(), &vec!["--version"]);
            } else {
                panic!("Expected ShellCommand constraint");
            }
        }
        Hook::Exec { exec, .. } => {
            // Check constraints in new format
            assert_eq!(exec.constraints.len(), 2);

            if let HookConstraint::CommandExists { command } = &exec.constraints[0] {
                assert_eq!(command, "devenv");
            } else {
                panic!("Expected CommandExists constraint");
            }

            if let HookConstraint::ShellCommand { command, args } = &exec.constraints[1] {
                assert_eq!(command, "nix");
                assert_eq!(args.as_ref().unwrap(), &vec!["--version"]);
            } else {
                panic!("Expected ShellCommand constraint");
            }
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    // Test onExit hook constraints
    let on_exit = &result.hooks.get("onExit").unwrap()[0];
    match on_exit {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "cleanup.sh");
            assert!(hook_config.args.is_empty());
            assert_eq!(hook_config.constraints.len(), 2);

            // Check first constraint - shell command
            if let HookConstraint::ShellCommand { command, args } = &hook_config.constraints[0]
            {
                assert_eq!(command, "test");
                assert_eq!(args.as_ref().unwrap(), &vec!["-f", "/tmp/cleanup_needed"]);
            } else {
                panic!("Expected ShellCommand constraint");
            }

            // Check second constraint - command exists
            if let HookConstraint::CommandExists { command } = &hook_config.constraints[1] {
                assert_eq!(command, "cleanup");
            } else {
                panic!("Expected CommandExists constraint");
            }
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "cleanup.sh");
            assert!(exec.args.as_ref().is_none_or(|args| args.is_empty()));
            assert_eq!(exec.constraints.len(), 2);

            // Check first constraint - shell command
            if let HookConstraint::ShellCommand { command, args } = &exec.constraints[0] {
                assert_eq!(command, "test");
                assert_eq!(args.as_ref().unwrap(), &vec!["-f", "/tmp/cleanup_needed"]);
            } else {
                panic!("Expected ShellCommand constraint");
            }

            // Check second constraint - command exists
            if let HookConstraint::CommandExists { command } = &exec.constraints[1] {
                assert_eq!(command, "cleanup");
            } else {
                panic!("Expected CommandExists constraint");
            }
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}

#[test]
#[serial]
fn test_parse_hooks_with_no_constraints() {
    let content = r#"
    package env

    hooks: {
        onEnter: {
            command: "echo"
            args: ["No constraints"]
        }
    }

    env: {
        API_KEY: "secret123"
    }
    "#;
    let temp_dir = create_test_env(content);
    let options = ParseOptions::default();
    let result =
        CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert_eq!(result.hooks.len(), 1);
    let hook = &result.hooks.get("onEnter").unwrap()[0];
    match hook {
        Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "echo");
            assert_eq!(hook_config.args, vec!["No constraints"]);
            assert!(hook_config.constraints.is_empty());
        }
        Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "echo");
            assert_eq!(exec.args.as_ref().unwrap(), &vec!["No constraints"]);
            assert!(exec.constraints.is_empty());
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}
