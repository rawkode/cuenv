use async_trait::async_trait;
use cuenv::command_executor::CommandExecutor;
use cuenv::errors::{Error, Result};
use cuenv::secrets::{CommandResolver, SecretManager};
use cuenv::types::{CommandArguments, EnvironmentVariables};
use std::collections::HashMap;
use std::fs;
use std::process::Output;
use tempfile::TempDir;

// Helper function to create ExitStatus
fn exit_status_from_code(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

/// Test executor that simulates 1Password CLI responses
struct OnePasswordTestExecutor;

#[async_trait]
impl CommandExecutor for OnePasswordTestExecutor {
    async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
        let args_slice = args.as_slice();
        if cmd == "op" && args_slice.len() >= 2 && args_slice[0] == "read" {
            let reference = &args_slice[1];

            match reference.as_str() {
                "op://rawkode.cuenv/test-password/password" => Ok(Output {
                    status: exit_status_from_code(0),
                    stdout: b"my-super-secret-password".to_vec(),
                    stderr: Vec::new(),
                }),
                _ => Ok(Output {
                    status: exit_status_from_code(1),
                    stdout: Vec::new(),
                    stderr: format!("[ERROR] item \"{reference}\" not found\n").into_bytes(),
                }),
            }
        } else if cmd == "echo" {
            // Support echo command for testing
            Ok(Output {
                status: exit_status_from_code(0),
                stdout: format!("{}\n", args_slice.join(" ")).into_bytes(),
                stderr: Vec::new(),
            })
        } else {
            Err(Error::command_execution(
                cmd,
                args_slice.to_vec(),
                format!("Unexpected command: {cmd} {args_slice:?}"),
                None,
            ))
        }
    }

    async fn execute_with_env(
        &self,
        cmd: &str,
        args: &CommandArguments,
        _env: EnvironmentVariables,
    ) -> Result<Output> {
        self.execute(cmd, args).await
    }
}

#[test]
fn test_cue_file_with_onepassword_secrets() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Write a CUE file with 1Password references
    fs::write(
        &env_file,
        r#"package cuenv

env: {
import "github.com/rawkode/cuenv/onepassword"

// Database configuration with 1Password secret
DB_HOST: "postgres.example.com"
DB_PORT: "5432"
DB_USER: "appuser"
DB_PASSWORD: onepassword.#OnePasswordRef & {
    ref: "op://rawkode.cuenv/test-password/password"
}

// API configuration
API_ENDPOINT: "https://api.example.com"

// Regular environment variables
NODE_ENV: "production"
LOG_LEVEL: "info"
}
"#,
    )
    .unwrap();

    // Note: We can't actually test the full integration here without modifying
    // EnvManager to accept a custom CommandExecutor, which would be part of
    // the builder pattern implementation (Task 4).

    // For now, we'll just verify the CUE file is written correctly
    assert!(env_file.exists());
    let content = fs::read_to_string(&env_file).unwrap();
    assert!(content.contains("op://rawkode.cuenv/test-password/password"));
}

#[test]
fn test_cue_file_with_inline_resolver_format() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    // Alternative CUE format using direct resolver structure
    fs::write(
        &env_file,
        r#"package cuenv

env: {
// Using the resolver structure directly without import
DB_PASSWORD: {
    resolver: {
        command: "op"
        args: ["read", "op://rawkode.cuenv/test-password/password"]
    }
}

// Mixed with regular values
APP_NAME: "my-app"
APP_VERSION: "1.0.0"
}
"#,
    )
    .unwrap();

    assert!(env_file.exists());
    let content = fs::read_to_string(&env_file).unwrap();
    assert!(content.contains("resolver:"));
    assert!(content.contains("command: \"op\""));
    assert!(content.contains("op://rawkode.cuenv/test-password/password"));
}

#[tokio::test]
async fn test_onepassword_integration_with_cue() {
    let temp_dir = TempDir::new().unwrap();

    // Create a CUE file with 1Password references using the resolver format
    let env_file = temp_dir.path().join("env.cue");
    fs::write(
        &env_file,
        r#"package cuenv

env: {
import (
    "github.com/rawkode/cuenv/pkg/secret/v1"
)

DB_PASSWORD: v1.#Resolver & {
    command: "op"
    args: ["read", "op://rawkode.cuenv/test-password/password"]
}

APP_NAME: "test-app"
}
"#,
    )
    .unwrap();

    // Create a command resolver with our test executor
    let test_executor = Box::new(OnePasswordTestExecutor);
    let resolver = CommandResolver::with_executor(10, test_executor);

    // Create a secret manager with our custom resolver
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    // Test the secret resolution directly
    let mut env_vars = HashMap::new();
    env_vars.insert(
        "DB_PASSWORD".to_string(),
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://rawkode.cuenv/test-password/password"]}"#.to_string()
    );
    env_vars.insert("APP_NAME".to_string(), "test-app".to_string());

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await
        .unwrap();

    // Verify the secrets were resolved correctly
    assert_eq!(
        resolved.env_vars.get("DB_PASSWORD").unwrap(),
        "my-super-secret-password"
    );
    assert_eq!(resolved.env_vars.get("APP_NAME").unwrap(), "test-app");

    // Verify secret values are tracked
    assert!(resolved.secret_values.contains("my-super-secret-password"));
    assert!(!resolved.secret_values.contains("test-app"));
}
