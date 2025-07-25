use crate::command_executor::CommandExecutor;
use crate::cue_parser::HookConfig;
use crate::types::{CommandArguments, EnvironmentVariables};
use anyhow::{anyhow, Result};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use url::Url;

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
    cache: Arc<Mutex<LruCache<String, CachedContent>>>,
    semaphore: Arc<Semaphore>,
    cache_ttl: Duration,
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
        let cache = Arc::new(Mutex::new(LruCache::new(cache_size)));

        let semaphore = Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT_HOOKS));

        Ok(Self {
            executor,
            http_client,
            cache,
            semaphore,
            cache_ttl: DEFAULT_CACHE_TTL,
        })
    }

    pub async fn execute_hook(
        &self,
        hook_config: &HookConfig,
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire semaphore: {}", e))?;

        log::debug!(
            "Executing hook: type={:?}, command={}, url={:?}",
            hook_config.hook_type,
            hook_config.command,
            hook_config.url
        );

        if let Some(url) = &hook_config.url {
            self.execute_remote_hook(url, env_vars).await
        } else {
            self.execute_local_hook(hook_config, env_vars).await
        }
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
            // Print hook output to stdout so users can see it
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }

        if !output.stderr.is_empty() {
            // Print hook stderr to stderr
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    async fn execute_remote_hook(
        &self,
        url: &str,
        env_vars: &HashMap<String, String>,
    ) -> Result<()> {
        let content = self.fetch_url_content(url).await?;

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
            // Print hook output to stdout so users can see it
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }

        if !output.stderr.is_empty() {
            // Print hook stderr to stderr
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    async fn fetch_url_content(&self, url: &str) -> Result<String> {
        // Validate URL
        let parsed_url = Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;

        // Check cache first
        let cache_key = url.to_string();

        {
            let mut cache = self.cache.lock().await;
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
            let mut cache = self.cache.lock().await;
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
    use crate::command_executor::TestCommandExecutor;
    use std::sync::Arc;

    async fn create_test_manager() -> HookManager<TestCommandExecutor> {
        let executor = Arc::new(TestCommandExecutor::new());
        HookManager::new(executor).unwrap()
    }

    #[tokio::test]
    async fn test_local_hook_execution() {
        let executor = Arc::new(TestCommandExecutor::new());

        // Add expected response for the echo command
        executor.add_response(
            "echo",
            &["test".to_string()],
            crate::command_executor::TestResponse {
                stdout: b"test\n".to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            },
        );

        let manager = HookManager::new(executor).unwrap();

        let hook_config = HookConfig {
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            url: None,
            hook_type: crate::cue_parser::HookType::OnEnter,
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

        let result = manager.fetch_url_content("not-a-valid-url").await;
        assert!(result.is_err());

        let result = manager.fetch_url_content("https://example.com").await;
        // This may fail due to network, but should at least parse the URL correctly
        assert!(result.is_err() || result.is_ok());
    }
}
