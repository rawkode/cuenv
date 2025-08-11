//! Integration tests for circuit breaker functionality.
//!
//! These tests focus on testing the interaction between different modules
//! and overall circuit breaker behavior.

#[cfg(test)]
mod integration_tests {
    use super::super::{
        config::{CircuitBreakerConfig, RetryConfig},
        retry::{retry_with_circuit_breaker, suggest_recovery},
        state::CircuitBreaker,
        types::{CircuitState, RetryOn},
    };
    use cuenv_core::{Error, Result};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_full_circuit_breaker_lifecycle() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            break_duration: Duration::from_millis(100),
            timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        };

        let cb = CircuitBreaker::new(config);
        let counter = Arc::new(AtomicUsize::new(0));
        let should_fail = Arc::new(AtomicBool::new(true));

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Cause failures to open circuit
        for _ in 0..2 {
            let _: Result<()> = cb
                .call(|| async {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err(Error::network("test", "fail"))
                })
                .await;
        }

        // Should be open now
        assert_eq!(cb.state().await, CircuitState::Open);

        // Calls should fail immediately
        let result: Result<()> = cb
            .call(|| async {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("open"));

        // Wait for break duration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Now allow successes
        should_fail.store(false, Ordering::SeqCst);

        // Successful calls should close the circuit
        for _ in 0..2 {
            let _: Result<()> = cb
                .call(|| async {
                    if should_fail.load(Ordering::SeqCst) {
                        Err(Error::network("test", "fail"))
                    } else {
                        Ok(())
                    }
                })
                .await;
        }

        // Should be closed again
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_retry_config_integration() {
        let retry_config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            retry_on: RetryOn::Network,
            ..Default::default()
        };

        let cb_config = CircuitBreakerConfig {
            failure_threshold: 5, // High threshold so circuit doesn't open
            ..Default::default()
        };

        let cb = CircuitBreaker::new(cb_config);
        let attempt_counter = Arc::new(AtomicUsize::new(0));

        let result = retry_with_circuit_breaker(&retry_config, &cb, || {
            let count = attempt_counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(Error::network("test", "temporary failure"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 3);
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_stats() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Initial stats
        let stats = cb.stats().await;
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_count, 0);

        // Trigger some failures
        for _ in 0..2 {
            let _: Result<()> = cb
                .call(|| async { Err(Error::network("test", "fail")) })
                .await;
        }

        let stats = cb.stats().await;
        assert_eq!(stats.failure_count, 2);
        assert_eq!(stats.state, CircuitState::Closed); // Not open yet

        // One more failure should open it
        let _: Result<()> = cb
            .call(|| async { Err(Error::network("test", "fail")) })
            .await;

        let stats = cb.stats().await;
        assert_eq!(stats.state, CircuitState::Open);
        assert!(stats.last_failure_time.is_some());
    }

    #[test]
    fn test_comprehensive_recovery_suggestions() {
        let test_cases = vec![
            (
                Error::network("api.example.com", "connection refused"),
                "internet connection",
            ),
            (
                Error::file_system(
                    std::path::PathBuf::from("/tmp/test"),
                    "read",
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
                ),
                "permissions",
            ),
            (Error::configuration("invalid syntax"), "env.cue"),
            (
                Error::timeout("operation", Duration::from_secs(30)),
                "timed out",
            ),
        ];

        for (error, expected_keyword) in test_cases {
            let suggestion = suggest_recovery(&error);
            assert!(
                suggestion.to_lowercase().contains(expected_keyword),
                "Suggestion '{suggestion}' should contain '{expected_keyword}'"
            );
        }
    }

    #[tokio::test]
    async fn test_retry_success() {
        use super::super::{config::RetryConfig, retry::retry};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = RetryConfig::default();
        let counter = Arc::new(AtomicUsize::new(0));

        let result = retry(&config, || {
            let count = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(Error::network("test", "simulated failure"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_max_attempts() {
        use super::super::{config::RetryConfig, retry::retry};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = RetryConfig {
            max_retries: 2,
            ..Default::default()
        };

        let counter = Arc::new(AtomicUsize::new(0));

        let result: Result<()> = retry(&config, || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Err(Error::network("test", "always fails")) }
        })
        .await;

        assert!(result.is_err());
        // Initial attempt + 2 retries = 3 total
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        use super::super::{config::RetryConfig, retry::retry};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let config = RetryConfig {
            retry_on: RetryOn::Network,
            ..Default::default()
        };

        let counter = Arc::new(AtomicUsize::new(0));

        let result: Result<()> = retry(&config, || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Err(Error::configuration("not retryable")) }
        })
        .await;

        assert!(result.is_err());
        // Should only try once
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_with_circuit_breaker_detailed() {
        let retry_config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        };

        let cb_config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(cb_config);

        let counter = Arc::new(AtomicUsize::new(0));

        // First, cause circuit to open
        for _ in 0..3 {
            let _: Result<()> = retry_with_circuit_breaker(&retry_config, &cb, || {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Err(Error::network("test", "fail")) }
            })
            .await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Now calls should fail immediately without retrying
        let before_count = counter.load(Ordering::SeqCst);

        let result = retry_with_circuit_breaker(&retry_config, &cb, || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Ok("would succeed") }
        })
        .await;

        assert!(result.is_err());
        // Should not have incremented counter (circuit open)
        assert_eq!(counter.load(Ordering::SeqCst), before_count);
    }

    #[test]
    fn test_suggest_recovery_detailed() {
        use super::super::retry::suggest_recovery;

        let network_err = Error::network("api.example.com", "connection refused");
        assert!(suggest_recovery(&network_err).contains("internet connection"));

        let fs_err = Error::file_system(
            std::path::PathBuf::from("/tmp/test"),
            "read",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        assert!(suggest_recovery(&fs_err).contains("permissions"));

        let config_err = Error::configuration("invalid syntax");
        assert!(suggest_recovery(&config_err).contains("env.cue"));
    }

    #[test]
    fn test_retry_on_variants() {
        let network_config = RetryConfig {
            retry_on: RetryOn::Network,
            ..Default::default()
        };

        let fs_config = RetryConfig {
            retry_on: RetryOn::FileSystem,
            ..Default::default()
        };

        let all_config = RetryConfig {
            retry_on: RetryOn::All,
            ..Default::default()
        };

        let network_error = Error::network("test", "fail");
        let fs_error = Error::file_system(
            std::path::PathBuf::from("/tmp/test"),
            "read",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        let config_error = Error::configuration("invalid");

        // Network config should only retry network errors
        assert!(network_config.should_retry(&network_error));
        assert!(!network_config.should_retry(&fs_error));
        assert!(!network_config.should_retry(&config_error));

        // FS config should only retry filesystem errors
        assert!(!fs_config.should_retry(&network_error));
        assert!(fs_config.should_retry(&fs_error));
        assert!(!fs_config.should_retry(&config_error));

        // All config should retry everything
        assert!(all_config.should_retry(&network_error));
        assert!(all_config.should_retry(&fs_error));
        assert!(all_config.should_retry(&config_error));
    }
}
