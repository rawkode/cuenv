use async_trait::async_trait;
use cuenv::command_executor::CommandExecutor;
use cuenv::errors::{Error, Result};
use cuenv::secrets::{CommandResolver, SecretManager};
use cuenv::types::{CommandArguments, EnvironmentVariables};
use proptest::prelude::*;
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

/// Echo command executor for testing custom resolvers
struct EchoExecutor;

#[async_trait]
impl CommandExecutor for EchoExecutor {
    async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
        if cmd == "echo" {
            // Echo command returns all arguments joined with spaces
            let output = args.as_slice().join(" ");
            Ok(Output {
                status: exit_status_from_code(0),
                stdout: output.into_bytes(),
                stderr: Vec::new(),
            })
        } else {
            Err(Error::command_execution(
                cmd,
                args.clone().into_inner(),
                format!("Unsupported command: {}", cmd),
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn test_echo_resolver_any_string(s in any::<String>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_any_bytes(bytes in any::<Vec<u8>>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Convert bytes to string, using replacement for invalid UTF-8
            let s = String::from_utf8_lossy(&bytes);
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_multiple_args(args in prop::collection::vec(any::<String>(), 0..10)) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            test_echo_resolver_multi_args_impl(args).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_deeply_nested_json(
        depth in 0..5usize,
        width in 1..5usize,
        content in "[a-zA-Z0-9]{1,10}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Generate a deeply nested string
            let mut s = content.clone();
            for _ in 0..depth {
                let parts = vec![s.clone(); width];
                s = parts.join("-");
            }
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_string_length(len in 0..1000usize) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let s = "a".repeat(len);
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_mixed_whitespace(
        prefix in "[ \t\n\r]*",
        content in "[^ \t\n\r]+",
        suffix in "[ \t\n\r]*"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let s = format!("{}{}{}", prefix, content, suffix);
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }

    #[test]
    fn test_echo_resolver_control_chars(s in "[\x00-\x1F\x7F]*") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            test_echo_resolver_impl(&s).await
        }).unwrap();
    }
}

async fn test_echo_resolver_impl(test_string: &str) -> anyhow::Result<()> {
    // Create echo resolver
    let echo_executor = Box::new(EchoExecutor);
    let resolver = CommandResolver::with_executor(10, echo_executor);
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    // Create proper JSON object
    let json_obj = serde_json::json!({
        "cmd": "echo",
        "args": [test_string]
    });

    let resolver_ref = format!("cuenv-resolver://{}", json_obj);

    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), resolver_ref.clone());
    env_vars.insert("NORMAL_VAR".to_string(), "normal".to_string());

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await?;

    // Verify the echo resolver returned the correct value
    let resolved_value = resolved
        .env_vars
        .get("TEST_VAR")
        .ok_or_else(|| anyhow::anyhow!("TEST_VAR not found in resolved env"))?;

    // The resolver trims whitespace from command output
    let expected_value = test_string.trim();
    assert_eq!(
        resolved_value, expected_value,
        "Echo resolver should return trimmed input. Input: {:?}, Expected (trimmed): {:?}, Got: {:?}",
        test_string, expected_value, resolved_value
    );

    // Verify it's tracked as a secret (using the trimmed value)
    assert!(
        resolved.secret_values.contains(expected_value),
        "Resolved value should be tracked as a secret"
    );

    // Verify normal var passed through
    assert_eq!(resolved.env_vars.get("NORMAL_VAR").unwrap(), "normal");

    Ok(())
}

async fn test_echo_resolver_multi_args_impl(args: Vec<String>) -> anyhow::Result<()> {
    // Create echo resolver
    let echo_executor = Box::new(EchoExecutor);
    let resolver = CommandResolver::with_executor(10, echo_executor);
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    // Create resolver reference with multiple args
    let json_obj = serde_json::json!({
        "cmd": "echo",
        "args": args
    });

    let resolver_ref = format!("cuenv-resolver://{}", json_obj);

    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), resolver_ref);

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await?;

    // Echo should join args with spaces, and resolver trims the output
    let expected = args.join(" ").trim().to_string();
    let resolved_value = resolved
        .env_vars
        .get("TEST_VAR")
        .ok_or_else(|| anyhow::anyhow!("TEST_VAR not found"))?;

    assert_eq!(
        resolved_value, &expected,
        "Echo resolver should join args with spaces and trim. Args: {:?}, Expected (trimmed): {:?}, Got: {:?}",
        args, expected, resolved_value
    );

    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_dynamic_custom_resolvers(
        command in "[a-z]{3,10}",
        args in prop::collection::vec(any::<String>(), 0..5),
        transform in prop::sample::select(vec!["upper", "lower", "reverse", "repeat", "trim"])
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            test_dynamic_resolver_impl(command, args, transform).await
        }).unwrap();
    }
}

async fn test_dynamic_resolver_impl(
    command: String,
    args: Vec<String>,
    transform: &str,
) -> anyhow::Result<()> {
    struct DynamicExecutor {
        command: String,
        transform: String,
    }

    #[async_trait]
    impl CommandExecutor for DynamicExecutor {
        async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
            if cmd == &self.command {
                let input = args.as_slice().join(" ");
                let output = match self.transform.as_str() {
                    "upper" => input.to_uppercase(),
                    "lower" => input.to_lowercase(),
                    "reverse" => input.chars().rev().collect(),
                    "repeat" => input.repeat(2),
                    "trim" => input.trim().to_string(),
                    _ => input,
                };
                Ok(Output {
                    status: exit_status_from_code(0),
                    stdout: output.into_bytes(),
                    stderr: Vec::new(),
                })
            } else {
                Err(Error::command_execution(
                    cmd,
                    args.clone().into_inner(),
                    format!("Unsupported command: {}", cmd),
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

    let executor = Box::new(DynamicExecutor {
        command: command.clone(),
        transform: transform.to_string(),
    });

    let resolver = CommandResolver::with_executor(10, executor);
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    // Create resolver reference
    let json_obj = serde_json::json!({
        "cmd": command,
        "args": args
    });

    let resolver_ref = format!("cuenv-resolver://{}", json_obj);

    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), resolver_ref);

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await?;

    // Verify it was resolved
    let resolved_value = resolved
        .env_vars
        .get("TEST_VAR")
        .ok_or_else(|| anyhow::anyhow!("TEST_VAR not found"))?;

    // Apply the transform to verify
    let expected_input = args.join(" ");
    let transformed = match transform {
        "upper" => expected_input.to_uppercase(),
        "lower" => expected_input.to_lowercase(),
        "reverse" => expected_input.chars().rev().collect(),
        "repeat" => expected_input.repeat(2),
        "trim" => expected_input.trim().to_string(),
        _ => expected_input,
    };

    // The resolver always trims the final output
    let expected = transformed.trim();

    assert_eq!(resolved_value, expected);
    Ok(())
}

#[tokio::test]
async fn test_custom_uppercase_resolver() {
    /// Uppercase converter resolver
    struct UppercaseExecutor;

    #[async_trait]
    impl CommandExecutor for UppercaseExecutor {
        async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
            if cmd == "uppercase" && !args.is_empty() {
                let input = args.as_slice().join(" ");
                let uppercased = input.to_uppercase();
                Ok(Output {
                    status: exit_status_from_code(0),
                    stdout: uppercased.into_bytes(),
                    stderr: Vec::new(),
                })
            } else {
                Err(Error::command_execution(
                    cmd,
                    args.clone().into_inner(),
                    "Invalid uppercase command".to_string(),
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

    // Create uppercase resolver
    let uppercase_executor = Box::new(UppercaseExecutor);
    let resolver = CommandResolver::with_executor(10, uppercase_executor);
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    let mut env_vars = HashMap::new();
    env_vars.insert(
        "UPPERCASED_SECRET".to_string(),
        r#"cuenv-resolver://{"cmd":"uppercase","args":["hello world"]}"#.to_string(),
    );

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await
        .unwrap();

    assert_eq!(
        resolved.env_vars.get("UPPERCASED_SECRET").unwrap(),
        "HELLO WORLD"
    );
}

#[tokio::test]
async fn test_resolver_concurrency() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    /// Slow echo executor to test concurrency
    struct SlowEchoExecutor {
        concurrent_count: Arc<AtomicUsize>,
        max_concurrent: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl CommandExecutor for SlowEchoExecutor {
        async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
            if cmd == "echo" {
                // Track concurrent executions
                let current = self.concurrent_count.fetch_add(1, Ordering::SeqCst) + 1;
                let max = self.max_concurrent.load(Ordering::SeqCst);
                if current > max {
                    self.max_concurrent.store(current, Ordering::SeqCst);
                }

                // Simulate slow operation
                sleep(Duration::from_millis(10)).await;

                let output = args.as_slice().join(" ");
                self.concurrent_count.fetch_sub(1, Ordering::SeqCst);

                Ok(Output {
                    status: exit_status_from_code(0),
                    stdout: output.into_bytes(),
                    stderr: Vec::new(),
                })
            } else {
                Err(Error::command_execution(
                    cmd,
                    args.clone().into_inner(),
                    "Unsupported command".to_string(),
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

    let concurrent_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));

    let slow_executor = Box::new(SlowEchoExecutor {
        concurrent_count: concurrent_count.clone(),
        max_concurrent: max_concurrent.clone(),
    });

    // Create resolver with limited concurrency
    let resolver = CommandResolver::with_executor(3, slow_executor);
    let secret_manager = SecretManager::with_resolver(Box::new(resolver));

    // Create many secrets to resolve
    let mut env_vars = HashMap::new();
    for i in 0..10 {
        env_vars.insert(
            format!("SECRET_{}", i),
            format!(
                r#"cuenv-resolver://{{"cmd":"echo","args":["secret-{}"]}}"#,
                i
            ),
        );
    }

    let resolved = secret_manager
        .resolve_secrets(EnvironmentVariables::from(env_vars))
        .await
        .unwrap();

    // Verify all secrets were resolved
    assert_eq!(resolved.env_vars.len(), 10);
    for i in 0..10 {
        assert_eq!(
            resolved.env_vars.get(&format!("SECRET_{}", i)).unwrap(),
            &format!("secret-{}", i)
        );
    }

    // Verify concurrency was limited
    let max = max_concurrent.load(Ordering::SeqCst);
    assert!(max <= 3, "Max concurrent executions {} should be <= 3", max);
}
