use anyhow::{anyhow, Result};
use cuenv_config::{HookConfig, HookConstraint};
use cuenv_core::types::{CommandArguments, EnvironmentVariables};
// TODO: Add SecurityValidator when moved from src/security.rs
// use cuenv_security::SecurityValidator;
use cuenv_security::{audit_logger, AuditLogger};
use cuenv_task::CommandExecutor;
use cuenv_utils::network::rate_limit::RateLimitManager;
use cuenv_utils::network::retry::RetryConfig;
use cuenv_utils::resilience::{CircuitBreaker, CircuitBreakerConfig};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use url::Url;

use cuenv_utils::hooks_status::HooksStatusManager;

const DEFAULT_CACHE_SIZE: usize = 100;
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_CONCURRENT_HOOKS: usize = 10;
const DEFAULT_USER_AGENT: &str = "cuenv/0.1";

struct CachedContent {
    content: String,
    fetched_at: Instant,
}

impl CachedContent {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.fetched_at.elapsed() > ttl
    }
}

pub struct HookManager<E: CommandExecutor + Send + Sync> {
    executor: Arc<E>,
    http_client: reqwest::Client,
    cache: Arc<RwLock<LruCache<String, CachedContent>>>,
    semaphore: Arc<Semaphore>,
    cache_ttl: Duration,
    rate_limiter: Option<Arc<RateLimitManager>>,
    audit_logger: Option<Arc<AuditLogger>>,
    _circuit_breaker: Arc<CircuitBreaker>,
    _retry_config: RetryConfig,
    status_manager: Option<Arc<HooksStatusManager>>,
}

impl<E: CommandExecutor + Send + Sync> HookManager<E> {
    pub fn new(executor: Arc<E>) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        let cache_size =
            NonZeroUsize::new(DEFAULT_CACHE_SIZE).ok_or_else(|| anyhow!("Invalid cache size"))?;
        let cache = Arc::new(RwLock::new(LruCache::new(cache_size)));

        let semaphore = Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT_HOOKS));

        let circuit_breaker_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(300),       // 5 minutes
            break_duration: Duration::from_secs(60), // 1 minute
            half_open_max_calls: 3,
        };

        let retry_config = RetryConfig::network();

        Ok(Self {
            executor,
            http_client,
            cache,
            semaphore,
            cache_ttl: DEFAULT_CACHE_TTL,
            rate_limiter: None,
            audit_logger: audit_logger(),
            _circuit_breaker: Arc::new(CircuitBreaker::new(circuit_breaker_config)),
            _retry_config: retry_config,
            status_manager: None,
        })
    }

    /// Set the rate limiter for hook execution
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<RateLimitManager>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Set the status manager for progress tracking
    pub fn with_status_manager(mut self, status_manager: Arc<HooksStatusManager>) -> Self {
        self.status_manager = Some(status_manager);
        self
    }

    /// Execute multiple hooks with progress tracking
    pub async fn execute_hooks(
        &self,
        hooks: &[HookConfig],
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        // Initialize status tracking if available
        if let Some(ref status_manager) = self.status_manager {
            let hook_names: Vec<String> = hooks
                .iter()
                .map(|h| format!("{:?}:{}", h.hook_type, h.command))
                .collect();
            let _ = status_manager.initialize_hooks(hook_names);
        }

        // Execute hooks sequentially
        for hook in hooks {
            self.execute_hook(hook, env_vars).await?;
        }

        Ok(())
    }

    pub async fn execute_hook(
        &self,
        hook_config: &HookConfig,
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        // Check rate limit if configured
        let _rate_limit_permit = if let Some(ref rate_limiter) = self.rate_limiter {
            match rate_limiter.try_acquire("hooks").await {
                Ok(Some(permit)) => Some(permit),
                Ok(None) => None,
                Err(e) => {
                    // Log rate limit exceeded
                    if let Some(ref logger) = self.audit_logger {
                        let _ = logger.log_rate_limit("hooks", 0, 0, true).await;
                    }
                    return Err(anyhow!("Rate limit exceeded: {}", e));
                }
            }
        } else {
            None
        };

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire semaphore: {}", e))?;

        log::debug!(
            "Executing hook: type={:?}, command={}, url={:?}, constraints={:?}",
            hook_config.hook_type,
            hook_config.command,
            hook_config.url,
            hook_config.constraints
        );

        // Generate a hook name for status tracking
        let hook_name = format!("{:?}:{}", hook_config.hook_type, hook_config.command);

        // Mark hook as started in status manager
        if let Some(ref status_manager) = self.status_manager {
            let pid = std::process::id();
            let _ = status_manager.mark_hook_started(&hook_name, pid);
        }

        // Check constraints before executing hook
        if !self
            .check_constraints(&hook_config.constraints, env_vars)
            .await?
        {
            log::info!(
                "Skipping hook '{}' due to unmet constraints",
                hook_config.command
            );

            // Mark as completed (skipped due to constraints)
            if let Some(ref status_manager) = self.status_manager {
                let _ = status_manager.mark_hook_completed(&hook_name);
            }

            return Ok(());
        }

        let start_time = Instant::now();
        let result = if let Some(url) = &hook_config.url {
            self.execute_remote_hook(url, env_vars).await
        } else {
            self.execute_local_hook(hook_config, env_vars).await
        };
        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Update status based on result
        if let Some(ref status_manager) = self.status_manager {
            match &result {
                Ok(_) => {
                    let _ = status_manager.mark_hook_completed(&hook_name);
                }
                Err(e) => {
                    let _ = status_manager.mark_hook_failed(&hook_name, e.to_string());
                }
            }
        }

        // Log the hook execution
        if let Some(ref logger) = self.audit_logger {
            let _ = logger
                .log_hook_execution(
                    &format!("{:?}", hook_config.hook_type),
                    &hook_config.command,
                    &hook_config.args,
                    result.is_ok(),
                    duration_ms,
                )
                .await;
        }

        result
    }

    async fn execute_local_hook(
        &self,
        hook_config: &HookConfig,
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        let isolated_env = self.create_isolated_environment(env_vars);

        log::debug!(
            "Executing local command: {} {:?}",
            hook_config.command,
            hook_config.args
        );

        let args = CommandArguments::from_vec(hook_config.args.clone());
        let env = EnvironmentVariables::from_map(isolated_env);

        let output = self
            .executor
            .execute_with_env(&hook_config.command, &args, env)
            .await
            .map_err(|e| anyhow!("Hook execution failed: {}", e))?;

        if !output.stdout.is_empty() {
            // Log hook output
            tracing::info!(
                "Hook stdout: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }

        if !output.stderr.is_empty() {
            // Log hook stderr
            tracing::warn!(
                "Hook stderr: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        Ok(())
    }

    async fn execute_remote_hook(
        &self,
        url: &str,
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        // Validate URL is from allowed domains (should be configurable)
        let parsed_url = Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;

        // Example: Only allow HTTPS URLs
        if parsed_url.scheme() != "https" {
            return Err(anyhow!("Only HTTPS URLs are allowed for remote hooks"));
        }

        let content = self.fetch_url_content(url).await?;

        // TODO: Validate script content for security when SecurityValidator is available
        // SecurityValidator::validate_shell_expansion(&content)
        //     .map_err(|e| anyhow!("Remote script validation failed: {}", e))?;

        log::debug!("Executing remote content from URL: {url}");

        let isolated_env = self.create_isolated_environment(env_vars);

        // Execute the fetched content as a shell script
        let args = CommandArguments::from_vec(vec!["-c".to_string(), content]);
        let env = EnvironmentVariables::from_map(isolated_env);

        let output = self
            .executor
            .execute_with_env("sh", &args, env)
            .await
            .map_err(|e| anyhow!("Remote hook execution failed: {}", e))?;

        if !output.stdout.is_empty() {
            // Log hook output
            tracing::info!(
                "Hook stdout: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }

        if !output.stderr.is_empty() {
            // Log hook stderr
            tracing::warn!(
                "Hook stderr: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        Ok(())
    }

    async fn fetch_url_content(&self, url: &str) -> Result<String> {
        // Validate URL
        let parsed_url = Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;

        // Check cache first
        let cache_key = url.to_string();

        {
            let mut cache = self.cache.write().await;
            if let Some(cached) = cache.get(&cache_key) {
                if !cached.is_expired(self.cache_ttl) {
                    log::debug!("Using cached content for URL: {url}");
                    return Ok(cached.content.clone());
                }
            }
        }

        // Fetch from URL
        log::debug!("Fetching content from URL: {url}");

        let response = self
            .http_client
            .get(parsed_url.as_str())
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch URL: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(anyhow!("HTTP request failed with status: {status}"));
        }

        let content = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {e}"))?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.put(
                cache_key,
                CachedContent {
                    content: content.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(content)
    }

    fn create_isolated_environment(
        &self,
        env_vars: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut isolated_env = HashMap::new();

        // Copy essential system environment variables
        for key in ["PATH", "HOME", "USER", "SHELL", "TERM"].iter() {
            if let Ok(value) = std::env::var(key) {
                isolated_env.insert(key.to_string(), value);
            }
        }

        // Add cuenv-managed environment variables
        for (key, value) in env_vars {
            isolated_env.insert(key.clone(), value.clone());
        }

        isolated_env
    }

    async fn check_constraints(
        &self,
        constraints: &[HookConstraint],
        env_vars: &HashMap<String, String>,
    ) -> Result<bool> {
        for constraint in constraints {
            if !self.check_single_constraint(constraint, env_vars).await? {
                log::debug!("Constraint not met: {constraint:?}");
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn check_single_constraint(
        &self,
        constraint: &HookConstraint,
        env_vars: &HashMap<String, String>,
    ) -> Result<bool> {
        match constraint {
            HookConstraint::CommandExists { command } => {
                self.check_command_exists(command, env_vars).await
            }
            HookConstraint::ShellCommand { command, args } => {
                self.check_shell_command(command, args.as_ref(), env_vars)
                    .await
            }
        }
    }

    async fn check_command_exists(
        &self,
        command: &str,
        env_vars: &HashMap<String, String>,
    ) -> Result<bool> {
        log::debug!("Checking if command '{command}' exists");

        let isolated_env = self.create_isolated_environment(env_vars);
        let args = CommandArguments::from_vec(vec![command.to_string()]);
        let env = EnvironmentVariables::from_map(isolated_env);

        // Use 'which' command to check if the command exists
        match self.executor.execute_with_env("which", &args, env).await {
            Ok(output) => {
                let exists = output.status.success();
                log::debug!("Command '{command}' exists: {exists}");
                Ok(exists)
            }
            Err(e) => {
                log::debug!("Failed to check command '{command}': {e}");
                Ok(false)
            }
        }
    }

    async fn check_shell_command(
        &self,
        command: &str,
        args: Option<&Vec<String>>,
        env_vars: &HashMap<String, String>,
    ) -> Result<bool> {
        log::debug!("Checking shell command: {command} {args:?}");

        let isolated_env = self.create_isolated_environment(env_vars);
        let command_args = CommandArguments::from_vec(args.cloned().unwrap_or_default());
        let env = EnvironmentVariables::from_map(isolated_env);

        match self
            .executor
            .execute_with_env(command, &command_args, env)
            .await
        {
            Ok(output) => {
                let success = output.status.success();
                log::debug!("Shell command '{command}' succeeded: {success}");
                Ok(success)
            }
            Err(e) => {
                log::debug!("Shell command '{command}' failed: {e}");
                Ok(false)
            }
        }
    }

    // Test helper methods
    #[cfg(test)]
    pub fn new_with_config(
        executor: Arc<E>,
        cache_ttl: Duration,
        max_concurrent: usize,
    ) -> Result<Self> {
        let mut manager = Self::new(executor)?;
        manager.cache_ttl = cache_ttl;
        manager.semaphore = Arc::new(Semaphore::new(max_concurrent));
        Ok(manager)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use cuenv_core::types::{CommandArguments, EnvironmentVariables};
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Type alias for the complex test response map type
    type TestResponseMap = Arc<RwLock<HashMap<(String, Vec<String>), TestResponse>>>;

    // Test-specific command executor
    #[derive(Debug)]
    struct TestResponse {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        status_code: i32,
    }

    #[derive(Debug)]
    struct TestCommandExecutor {
        responses: TestResponseMap,
    }

    impl TestCommandExecutor {
        fn new() -> Self {
            Self {
                responses: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        async fn add_response(&self, command: &str, args: &[String], response: TestResponse) {
            let key = (command.to_string(), args.to_vec());
            self.responses.write().await.insert(key, response);
        }
    }

    #[async_trait]
    impl cuenv_task::command_executor::CommandExecutor for TestCommandExecutor {
        async fn execute(
            &self,
            command: &str,
            args: &CommandArguments,
        ) -> cuenv_core::Result<Output> {
            let key = (command.to_string(), args.as_slice().to_vec());
            let responses = self.responses.read().await;

            if let Some(response) = responses.get(&key) {
                let output = Output {
                    status: if response.status_code == 0 {
                        ExitStatus::from_raw(0)
                    } else {
                        ExitStatus::from_raw(response.status_code)
                    },
                    stdout: response.stdout.clone(),
                    stderr: response.stderr.clone(),
                };
                Ok(output)
            } else {
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        }

        async fn execute_with_env(
            &self,
            cmd: &str,
            args: &CommandArguments,
            _env: EnvironmentVariables,
        ) -> cuenv_core::Result<Output> {
            // Just delegate to execute for tests
            self.execute(cmd, args).await
        }
    }

    async fn create_test_manager() -> HookManager<TestCommandExecutor> {
        let executor = Arc::new(TestCommandExecutor::new());
        HookManager::new(executor).unwrap()
    }

    #[tokio::test]
    async fn test_local_hook_execution() {
        let executor = Arc::new(TestCommandExecutor::new());

        // Add expected response for the echo command
        executor
            .add_response(
                "echo",
                &["test".to_string()],
                TestResponse {
                    stdout: b"test\n".to_vec(),
                    stderr: Vec::new(),
                    status_code: 0,
                },
            )
            .await;

        let manager = HookManager::new(executor).unwrap();

        let hook_config = HookConfig {
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            url: None,
            source: None,
            constraints: Vec::new(),
            hook_type: cuenv_config::HookType::OnEnter,
        };

        let env_vars = HashMap::new();
        let result = manager.execute_hook(&hook_config, &env_vars).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_isolated_environment_creation() {
        let manager = create_test_manager().await;

        let mut env_vars = HashMap::new();
        env_vars.insert("CUSTOM_VAR".to_string(), "custom_value".to_string());

        let isolated_env = manager.create_isolated_environment(&env_vars);

        assert_eq!(
            isolated_env.get("CUSTOM_VAR"),
            Some(&"custom_value".to_string())
        );
        assert!(isolated_env.contains_key("PATH"));
    }

    #[tokio::test]
    async fn test_url_validation() {
        let manager = create_test_manager().await;

        // Test invalid URL
        let result = manager.fetch_url_content("not-a-valid-url").await;
        assert!(result.is_err());

        // Test file:// URLs are rejected
        let result = manager.fetch_url_content("file:///etc/passwd").await;
        assert!(result.is_err());

        // Don't make actual network requests in tests
    }

    #[tokio::test]
    async fn test_constraint_command_exists() {
        let executor = Arc::new(TestCommandExecutor::new());

        // Mock 'which' command to return success for 'echo'
        executor
            .add_response(
                "which",
                &["echo".to_string()],
                TestResponse {
                    stdout: b"/bin/echo\n".to_vec(),
                    stderr: Vec::new(),
                    status_code: 0,
                },
            )
            .await;

        // Mock 'which' command to return failure for 'nonexistent'
        executor
            .add_response(
                "which",
                &["nonexistent".to_string()],
                TestResponse {
                    stdout: Vec::new(),
                    stderr: b"which: no nonexistent in (/usr/bin:/bin)\n".to_vec(),
                    status_code: 1,
                },
            )
            .await;

        let manager = HookManager::new(executor).unwrap();
        let env_vars = HashMap::new();

        // Test existing command
        let constraint = HookConstraint::CommandExists {
            command: "echo".to_string(),
        };
        let result = manager
            .check_single_constraint(&constraint, &env_vars)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test non-existing command
        let constraint = HookConstraint::CommandExists {
            command: "nonexistent".to_string(),
        };
        let result = manager
            .check_single_constraint(&constraint, &env_vars)
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_constraint_shell_command() {
        let executor = Arc::new(TestCommandExecutor::new());

        // Mock successful command
        executor
            .add_response(
                "true",
                &[],
                TestResponse {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    status_code: 0,
                },
            )
            .await;

        // Mock failing command
        executor
            .add_response(
                "false",
                &[],
                TestResponse {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    status_code: 1,
                },
            )
            .await;

        let manager = HookManager::new(executor).unwrap();
        let env_vars = HashMap::new();

        // Test successful shell command
        let constraint = HookConstraint::ShellCommand {
            command: "true".to_string(),
            args: None,
        };
        let result = manager
            .check_single_constraint(&constraint, &env_vars)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test failing shell command
        let constraint = HookConstraint::ShellCommand {
            command: "false".to_string(),
            args: None,
        };
        let result = manager
            .check_single_constraint(&constraint, &env_vars)
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_hook_skipped_when_constraints_not_met() {
        let executor = Arc::new(TestCommandExecutor::new());

        // Mock 'which' command to return failure for 'devenv'
        executor
            .add_response(
                "which",
                &["devenv".to_string()],
                TestResponse {
                    stdout: Vec::new(),
                    stderr: b"which: no devenv in (/usr/bin:/bin)\n".to_vec(),
                    status_code: 1,
                },
            )
            .await;

        let manager = HookManager::new(executor).unwrap();

        let hook_config = HookConfig {
            command: "devenv".to_string(),
            args: vec!["up".to_string()],
            url: None,
            source: None,
            constraints: vec![HookConstraint::CommandExists {
                command: "devenv".to_string(),
            }],
            hook_type: cuenv_config::HookType::OnEnter,
        };

        let env_vars = HashMap::new();
        // This should succeed (not error) but skip execution due to unmet constraint
        let result = manager.execute_hook(&hook_config, &env_vars).await;
        assert!(result.is_ok());
    }
}
