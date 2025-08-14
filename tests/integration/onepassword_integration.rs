use async_trait::async_trait;
use cuenv::command_executor::CommandExecutor;
use cuenv::errors::{Error, Result};
use cuenv::secrets::{CommandResolver, SecretManager, SecretResolver};
use cuenv::types::{CommandArguments, EnvironmentVariables};
use std::collections::HashMap;
use std::process::Output;

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
        // Simulate 1Password CLI behavior with op read
        let args_slice = args.as_slice();
        if cmd == "op" && args_slice.len() >= 2 && args_slice[0] == "read" {
            let reference = &args_slice[1];

            // Check for our test reference
            if reference == "op://rawkode.cuenv/test-password/password" {
                Ok(Output {
                    status: exit_status_from_code(0),
                    stdout: b"my-super-secret-password".to_vec(),
                    stderr: Vec::new(),
                })
            } else {
                // Simulate item not found
                Ok(Output {
                    status: exit_status_from_code(1),
                    stdout: Vec::new(),
                    stderr: format!("[ERROR] 2024/01/01 00:00:00 item \"{reference}\" not found\n")
                        .into_bytes(),
                })
            }
        } else {
            Err(Error::command_execution(
                cmd,
                args.clone().into_inner(),
                format!("Unexpected command: {} {:?}", cmd, args.as_slice()),
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

#[tokio::test]
async fn test_onepassword_secret_resolution() {
    // Create a CommandResolver with our test executor
    let resolver = CommandResolver::with_executor(10, Box::new(OnePasswordTestExecutor));

    // Test the exact reference format with op read command
    let reference = r#"cuenv-resolver://{"cmd":"op","args":["read","op://rawkode.cuenv/test-password/password"]}"#;

    let result = resolver.resolve(reference).await.unwrap();
    assert_eq!(result, Some("my-super-secret-password".to_string()));
}

#[tokio::test]
async fn test_onepassword_secret_manager_integration() {
    // Create a SecretManager with our test resolver
    let resolver = CommandResolver::with_executor(10, Box::new(OnePasswordTestExecutor));
    let manager = SecretManager::with_resolver(Box::new(resolver));

    // Create environment with 1Password references
    let mut env_vars = HashMap::new();
    env_vars.insert(
        "DB_PASSWORD".to_string(),
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://rawkode.cuenv/test-password/password"]}"#.to_string(),
    );
    env_vars.insert("DB_HOST".to_string(), "localhost".to_string());
    env_vars.insert("DB_PORT".to_string(), "5432".to_string());

    // Resolve secrets
    let resolved = manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await
        .unwrap();

    // Verify the secret was resolved
    assert_eq!(
        resolved.env_vars.get("DB_PASSWORD").unwrap(),
        "my-super-secret-password"
    );
    assert_eq!(resolved.env_vars.get("DB_HOST").unwrap(), "localhost");
    assert_eq!(resolved.env_vars.get("DB_PORT").unwrap(), "5432");

    // Verify the secret value is tracked for obfuscation
    assert!(resolved.secret_values.contains("my-super-secret-password"));
    assert_eq!(resolved.secret_values.len(), 1);
}

#[tokio::test]
async fn test_onepassword_error_handling() {
    let resolver = CommandResolver::with_executor(10, Box::new(OnePasswordTestExecutor));

    // Test with non-existent item
    let reference =
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://vault/nonexistent/field"]}"#;

    let result = resolver.resolve(reference).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("command failed"));
}

#[tokio::test]
async fn test_multiple_onepassword_secrets() {
    let resolver = CommandResolver::with_executor(10, Box::new(OnePasswordTestExecutor));
    let manager = SecretManager::with_resolver(Box::new(resolver));

    // Create environment with multiple secrets
    let mut env_vars = HashMap::new();

    // Add multiple 1Password references (only one will succeed in our test)
    env_vars.insert(
        "SECRET_1".to_string(),
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://rawkode.cuenv/test-password/password"]}"#.to_string(),
    );
    env_vars.insert(
        "SECRET_2".to_string(),
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://vault/other/field"]}"#.to_string(),
    );
    env_vars.insert("NORMAL_VAR".to_string(), "normal-value".to_string());

    let resolved = manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await
        .unwrap();

    // First secret should be resolved
    assert_eq!(
        resolved.env_vars.get("SECRET_1").unwrap(),
        "my-super-secret-password"
    );

    // Second secret should keep its original value due to error
    assert_eq!(
        resolved.env_vars.get("SECRET_2").unwrap(),
        r#"cuenv-resolver://{"cmd":"op","args":["read","op://vault/other/field"]}"#
    );

    // Normal var should pass through
    assert_eq!(resolved.env_vars.get("NORMAL_VAR").unwrap(), "normal-value");

    // Only successful secret should be in the set
    assert_eq!(resolved.secret_values.len(), 1);
    assert!(resolved.secret_values.contains("my-super-secret-password"));
}

// Import the platform-specific function
