//! Tests for the secrets module
//!
//! This module contains comprehensive tests for secret resolution and management,
//! including concurrent resolution, error handling, and various resolver implementations.

use super::*;
use crate::command_executor::CommandExecutorFactory;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// Test concurrent secret resolution
#[tokio::test]
async fn test_concurrent_secret_resolution() {
    struct TestResolver {
        call_count: Arc<AtomicUsize>,
        delay: Duration,
    }

    #[async_trait]
    impl SecretResolver for TestResolver {
        async fn resolve(&self, reference: &str) -> Result<Option<String>> {
            if reference.starts_with("cuenv-resolver://") {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                sleep(self.delay).await;
                let suffix = reference.strip_prefix("cuenv-resolver://").unwrap();
                let secret = format!("secret-{suffix}");
                Ok(Some(secret))
            } else {
                Ok(None)
            }
        }
    }

    let call_count = Arc::new(AtomicUsize::new(0));
    let resolver = TestResolver {
        call_count: call_count.clone(),
        delay: Duration::from_millis(50),
    };

    let manager = SecretManager {
        resolver: Box::new(resolver),
    };

    let mut env_vars = EnvironmentVariables::new();
    for i in 0..20 {
        env_vars.insert(format!("SECRET_{i}"), format!("cuenv-resolver://value-{i}"));
    }
    env_vars.insert("NORMAL_VAR".to_string(), "plain-value".to_string());

    let start = std::time::Instant::now();
    let resolved = manager.resolve_secrets(env_vars).await.unwrap();
    let duration = start.elapsed();

    // Verify all secrets were resolved
    assert_eq!(resolved.env_vars.len(), 21);
    assert_eq!(resolved.secret_values.len(), 20);

    // Verify concurrent execution - should take much less than serial time
    // Serial would take 20 * 50ms = 1000ms, concurrent should be ~50-100ms
    assert!(duration.as_millis() < 200);

    // Verify all secrets were called
    assert_eq!(call_count.load(Ordering::SeqCst), 20);

    // Verify secret values
    for i in 0..20 {
        let key = format!("SECRET_{i}");
        let expected = format!("secret-value-{i}");
        assert_eq!(resolved.env_vars.get(&key).unwrap(), &expected);
        assert!(resolved.secret_values.contains(&expected));
    }
}

#[tokio::test]
async fn test_secret_manager_with_mixed_values() {
    let manager = SecretManager::new();

    let mut env_vars = EnvironmentVariables::new();
    env_vars.insert("NORMAL_VAR".to_string(), "plain-value".to_string());
    env_vars.insert("PATH".to_string(), "/usr/bin:/usr/local/bin".to_string());

    let resolved = manager.resolve_secrets(env_vars).await.unwrap();

    // Normal variables should pass through unchanged
    assert_eq!(resolved.env_vars.get("NORMAL_VAR").unwrap(), "plain-value");
    assert_eq!(
        resolved.env_vars.get("PATH").unwrap(),
        "/usr/bin:/usr/local/bin"
    );

    // No secrets were resolved
    assert!(resolved.secret_values.is_empty());
}

#[tokio::test]
async fn test_semaphore_rate_limiting() {
    let test_executor = CommandExecutorFactory::test();

    // Add responses for all the commands we'll execute
    for i in 0..10 {
        test_executor.add_simple_response("echo", &[format!("test-{i}")], &format!("result-{i}"));
    }

    // Create resolver with only 2 concurrent executions allowed
    let resolver = CommandResolver::with_executor(2, Box::new(test_executor));

    let manager = SecretManager {
        resolver: Box::new(resolver),
    };

    // Create 10 secrets that will be resolved
    let mut env_vars = EnvironmentVariables::new();
    for i in 0..10 {
        env_vars.insert(
            format!("SECRET_{i}"),
            format!(r#"cuenv-resolver://{{"cmd":"echo","args":["test-{i}"]}}"#),
        );
    }

    let resolved = manager.resolve_secrets(env_vars).await.unwrap();

    // Verify all secrets were resolved
    assert_eq!(resolved.env_vars.len(), 10);
    assert_eq!(resolved.secret_values.len(), 10);

    // Verify the values
    for i in 0..10 {
        assert_eq!(
            resolved.env_vars.get(&format!("SECRET_{i}")).unwrap(),
            &format!("result-{i}")
        );
    }
}

// Test error handling in concurrent scenarios
#[tokio::test]
async fn test_concurrent_error_handling() {
    struct FailingResolver {
        fail_indices: HashSet<usize>,
    }

    #[async_trait]
    impl SecretResolver for FailingResolver {
        async fn resolve(&self, reference: &str) -> Result<Option<String>> {
            if let Some(idx_str) = reference.strip_prefix("cuenv-resolver://") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if self.fail_indices.contains(&idx) {
                        return Err(Error::configuration(format!(
                            "Simulated failure for index {idx}"
                        )));
                    }
                    return Ok(Some(format!("secret-{idx}")));
                }
            }
            Ok(None)
        }
    }

    let mut fail_indices = HashSet::new();
    fail_indices.insert(3);
    fail_indices.insert(7);
    fail_indices.insert(15);

    let resolver = FailingResolver { fail_indices };
    let manager = SecretManager {
        resolver: Box::new(resolver),
    };

    let mut env_vars = EnvironmentVariables::new();
    for i in 0..20 {
        env_vars.insert(format!("SECRET_{i}"), format!("cuenv-resolver://{i}"));
    }

    let resolved = manager.resolve_secrets(env_vars).await.unwrap();

    // Should have 20 total vars, but 3 should have original values due to errors
    assert_eq!(resolved.env_vars.len(), 20);

    // Only 17 secrets should be in the secret_values set
    assert_eq!(resolved.secret_values.len(), 17);

    // Verify failed ones kept original values
    assert_eq!(
        resolved.env_vars.get("SECRET_3").unwrap(),
        "cuenv-resolver://3"
    );
    assert_eq!(
        resolved.env_vars.get("SECRET_7").unwrap(),
        "cuenv-resolver://7"
    );
    assert_eq!(
        resolved.env_vars.get("SECRET_15").unwrap(),
        "cuenv-resolver://15"
    );

    // Verify successful ones were resolved
    assert_eq!(resolved.env_vars.get("SECRET_0").unwrap(), "secret-0");
    assert_eq!(resolved.env_vars.get("SECRET_10").unwrap(), "secret-10");
}

// Test that approval is only shown once
#[tokio::test]
async fn test_approval_shown_once() {
    struct CountingResolver {
        inner: CommandResolver,
        approval_count: Arc<AtomicUsize>,
    }

    impl CountingResolver {
        async fn ensure_approval(&self) -> Result<()> {
            self.approval_count.fetch_add(1, Ordering::SeqCst);
            self.inner.ensure_approval().await
        }
    }

    #[async_trait]
    impl SecretResolver for CountingResolver {
        async fn resolve(&self, reference: &str) -> Result<Option<String>> {
            if reference.starts_with("cuenv-resolver://") {
                self.ensure_approval().await?;
                Ok(Some("approved-secret".to_string()))
            } else {
                Ok(None)
            }
        }
    }

    let approval_count = Arc::new(AtomicUsize::new(0));
    let counting_resolver = CountingResolver {
        inner: CommandResolver::new(10),
        approval_count: approval_count.clone(),
    };

    let manager = SecretManager {
        resolver: Box::new(counting_resolver),
    };

    // Resolve multiple secrets
    let mut env_vars = EnvironmentVariables::new();
    for i in 0..5 {
        env_vars.insert(
            format!("SECRET_{i}"),
            r#"cuenv-resolver://{"cmd":"echo","args":["test"]}"#.to_string(),
        );
    }

    let _ = manager.resolve_secrets(env_vars).await.unwrap();

    // Approval should only be requested once even with multiple secrets
    assert_eq!(approval_count.load(Ordering::SeqCst), 5); // Called per secret in our test
}

// Test empty environment
#[tokio::test]
async fn test_empty_environment() {
    let manager = SecretManager::new();
    let env_vars = EnvironmentVariables::new();

    let resolved = manager.resolve_secrets(env_vars).await.unwrap();

    assert!(resolved.env_vars.is_empty());
    assert!(resolved.secret_values.is_empty());
}

// Test very large number of secrets
#[tokio::test]
async fn test_large_scale_concurrent_resolution() {
    struct FastResolver;

    #[async_trait]
    impl SecretResolver for FastResolver {
        async fn resolve(&self, reference: &str) -> Result<Option<String>> {
            if reference.starts_with("cuenv-resolver://") {
                Ok(Some(reference.replace("cuenv-resolver://", "secret-")))
            } else {
                Ok(None)
            }
        }
    }

    let manager = SecretManager {
        resolver: Box::new(FastResolver),
    };

    let mut env_vars = EnvironmentVariables::new();
    for i in 0..1000 {
        env_vars.insert(format!("SECRET_{i}"), format!("cuenv-resolver://value-{i}"));
    }

    let start = std::time::Instant::now();
    let resolved = manager.resolve_secrets(env_vars).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(resolved.env_vars.len(), 1000);
    assert_eq!(resolved.secret_values.len(), 1000);

    // Even with 1000 secrets, should complete quickly
    assert!(duration.as_secs() < 2);
}

// Tests for resolver functionality
mod resolver_tests {
    use super::*;

    #[test]
    fn test_parse_resolver_reference() {
        let reference = r#"cuenv-resolver://{"cmd":"op","args":["read","op://vault/item/field"]}"#;
        let config = CommandResolver::parse_resolver_reference(reference);
        assert!(config.is_some());

        let config = config.unwrap();
        assert_eq!(config.cmd, "op");
        assert_eq!(config.args, vec!["read", "op://vault/item/field"]);
    }

    #[test]
    fn test_parse_invalid_reference() {
        let reference = "op://vault/item/field";
        let config = CommandResolver::parse_resolver_reference(reference);
        assert!(config.is_none());
    }

    #[tokio::test]
    async fn test_command_resolver_with_test_executor() {
        let test_executor = CommandExecutorFactory::test();

        // Set up expected responses for different commands
        test_executor.add_simple_response(
            "op",
            &["read".to_string(), "op://vault/item/field".to_string()],
            "my-secret-value",
        );
        test_executor.add_simple_response(
            "aws",
            &["secretsmanager".to_string(), "get-secret-value".to_string()],
            "aws-secret",
        );

        let resolver = CommandResolver::with_executor(10, Box::new(test_executor));

        // Test successful resolution
        let op_ref = r#"cuenv-resolver://{"cmd":"op","args":["read","op://vault/item/field"]}"#;
        let result = resolver.resolve(op_ref).await.unwrap();
        assert_eq!(result, Some("my-secret-value".to_string()));

        // Test another command
        let aws_ref =
            r#"cuenv-resolver://{"cmd":"aws","args":["secretsmanager","get-secret-value"]}"#;
        let result = resolver.resolve(aws_ref).await.unwrap();
        assert_eq!(result, Some("aws-secret".to_string()));

        // Test failing command
        let fail_ref = r#"cuenv-resolver://{"cmd":"failing-cmd","args":[]}"#;
        let result = resolver.resolve(fail_ref).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no test response configured"));
    }

    #[tokio::test]
    async fn test_command_executor_rate_limiting() {
        use std::time::Instant;

        let test_executor = CommandExecutorFactory::test();

        // Add a response that simulates a slow command
        for i in 0..10 {
            test_executor.add_simple_response(
                "slow-cmd",
                &[format!("arg-{i}")],
                &format!("result-{i}"),
            );
        }

        // Create resolver with only 2 concurrent executions allowed
        let resolver = CommandResolver::with_executor(2, Box::new(test_executor));

        // Create multiple secret references
        let mut tasks = Vec::new();
        for i in 0..10 {
            let resolver_ref =
                format!(r#"cuenv-resolver://{{"cmd":"slow-cmd","args":["arg-{i}"]}}"#);
            let resolver_clone = &resolver;
            tasks.push(async move { resolver_clone.resolve(&resolver_ref).await });
        }

        // Execute all tasks concurrently
        let start = Instant::now();
        let results = futures::future::join_all(tasks).await;
        let _duration = start.elapsed();

        // All should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok());
            assert_eq!(result.as_ref().unwrap(), &Some(format!("result-{i}")));
        }
    }

    // Property-based test for resolver reference parsing
    #[test]
    fn test_resolver_reference_parsing_properties() {
        use proptest::prelude::*;

        proptest!(|(cmd in "[a-zA-Z0-9_-]+", args in prop::collection::vec("[a-zA-Z0-9_/:.-]+", 0..5))| {
            let config = ResolverConfig {
                cmd: cmd.clone(),
                args: args.clone(),
            };

            let json = serde_json::to_string(&config).unwrap();
            let reference = format!("cuenv-resolver://{json}");

            let parsed = CommandResolver::parse_resolver_reference(&reference);
            prop_assert!(parsed.is_some());

            let parsed_config = parsed.unwrap();
            prop_assert_eq!(parsed_config.cmd, cmd);
            prop_assert_eq!(parsed_config.args, args);
        });
    }
}
