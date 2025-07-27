//! Remote Execution API implementation
//!
//! This module implements the main remote execution functionality,
//! coordinating between action digests, CAS, cache, and sandboxed execution.

use crate::remote_cache::{
    proto::{Action, ActionResult, Command, Digest, ExecutionMetadata, OutputFile},
    ActionDigest, CASClient, CacheClient, DigestFunction, RemoteCacheError, RemoteCacheStats,
    Result, Sandbox, SandboxConfig, SandboxMode,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, Semaphore};

/// Remote executor configuration
#[derive(Debug, Clone)]
pub struct RemoteExecutorConfig {
    /// Base directory for execution
    pub exec_root: PathBuf,
    /// CAS client configuration
    pub cas_config: crate::remote_cache::CASClientConfig,
    /// Cache client configuration
    pub cache_config: crate::remote_cache::CacheClientConfig,
    /// Sandbox mode
    pub sandbox_mode: SandboxMode,
    /// Maximum concurrent executions
    pub max_concurrent_executions: usize,
    /// Digest function to use
    pub digest_function: DigestFunction,
    /// Worker ID
    pub worker_id: String,
    /// Enable detailed metrics
    pub enable_metrics: bool,
}

impl Default for RemoteExecutorConfig {
    fn default() -> Self {
        Self {
            exec_root: PathBuf::from(".cuenv/exec"),
            cas_config: Default::default(),
            cache_config: Default::default(),
            sandbox_mode: SandboxMode::default(),
            max_concurrent_executions: num_cpus::get(),
            digest_function: DigestFunction::SHA256,
            worker_id: format!(
                "worker-{}",
                hostname::get().unwrap_or_default().to_string_lossy()
            ),
            enable_metrics: true,
        }
    }
}

/// Remote executor
pub struct RemoteExecutor {
    config: RemoteExecutorConfig,
    cas_client: Arc<CASClient>,
    cache_client: Arc<CacheClient>,
    action_digest: Arc<ActionDigest>,
    execution_semaphore: Arc<Semaphore>,
    stats: Arc<ExecutorStats>,
}

impl RemoteExecutor {
    /// Create a new remote executor
    pub async fn new(config: RemoteExecutorConfig) -> Result<Self> {
        // Initialize CAS client
        let cas_client = Arc::new(CASClient::new(config.cas_config.clone()).await?);

        // Initialize cache client
        let cache_client =
            Arc::new(CacheClient::new(config.cache_config.clone(), cas_client.clone()).await?);

        // Initialize action digest builder
        let action_digest = Arc::new(ActionDigest::new(config.digest_function));

        // Create execution semaphore
        let execution_semaphore = Arc::new(Semaphore::new(config.max_concurrent_executions));

        Ok(Self {
            config,
            cas_client,
            cache_client,
            action_digest,
            execution_semaphore,
            stats: Arc::new(ExecutorStats::default()),
        })
    }

    /// Execute an action
    pub async fn execute_action(
        &self,
        command: Vec<String>,
        env_vars: HashMap<String, String>,
        working_dir: &Path,
        input_files: Vec<PathBuf>,
        output_files: Vec<String>,
        output_dirs: Vec<String>,
        timeout: Option<Duration>,
    ) -> Result<ActionResult> {
        let start_time = SystemTime::now();

        // Compute input root digest
        let input_patterns: Vec<String> = input_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let input_root_digest = self
            .action_digest
            .compute_input_root_digest(working_dir, &input_patterns)
            .await?;

        // Create command
        let (command_proto, command_digest) = self.action_digest.compute_command_digest(
            command,
            env_vars,
            output_files.clone(),
            output_dirs.clone(),
            ".".to_string(),
            Default::default(),
        )?;

        // Store command in CAS
        let command_data = serde_json::to_vec(&command_proto)?;
        self.cas_client.put(&command_data).await?;

        // Create action
        let action = Action {
            command_digest,
            input_root_digest,
            timeout,
            do_not_cache: false,
            platform: Default::default(),
        };

        // Compute action digest
        let action_data = serde_json::to_vec(&action)?;
        let action_digest_value = self.action_digest.compute_file_digest(&action_data);

        // Check cache
        if let Some(cached_result) = self
            .cache_client
            .get_action_result(&action_digest_value)
            .await?
        {
            self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached_result);
        }

        self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Acquire execution permit
        let _permit = self.execution_semaphore.acquire().await.map_err(|e| {
            RemoteCacheError::Configuration(format!("Failed to acquire permit: {}", e))
        })?;

        // Execute action
        let result = self
            .execute_action_internal(&action, &command_proto, working_dir)
            .await?;

        // Update cache
        self.cache_client
            .update_action_result(&action_digest_value, &action, &result)
            .await?;

        self.stats.actions_executed.fetch_add(1, Ordering::Relaxed);

        Ok(result)
    }

    /// Internal action execution
    async fn execute_action_internal(
        &self,
        action: &Action,
        command: &Command,
        working_dir: &Path,
    ) -> Result<ActionResult> {
        let execution_id = uuid::Uuid::new_v4().to_string();
        let exec_root = self.config.exec_root.join(&execution_id);

        // Create execution directory
        tokio::fs::create_dir_all(&exec_root).await?;

        // Set up execution environment
        let queued_timestamp = SystemTime::now();

        // Download input files
        let input_fetch_start = SystemTime::now();
        self.download_inputs(&action.input_root_digest, &exec_root)
            .await?;
        let input_fetch_completed = SystemTime::now();

        // Prepare sandbox
        let sandbox_config = SandboxConfig {
            mode: self.config.sandbox_mode,
            working_dir: exec_root.clone(),
            read_paths: vec![exec_root.clone()],
            write_paths: vec![exec_root.clone()],
            exec_paths: self.get_exec_paths(),
            env_vars: command
                .environment_variables
                .iter()
                .map(|ev| (ev.name.clone(), ev.value.clone()))
                .collect(),
            allow_network: false,
            ..Default::default()
        };

        let sandbox = Sandbox::new(sandbox_config);
        sandbox.prepare().await?;

        // Execute command
        let worker_start = SystemTime::now();
        let execution_start = SystemTime::now();

        let sandbox_result = sandbox.execute(command.arguments.clone(), None).await?;

        let execution_completed = SystemTime::now();

        // Upload outputs
        let output_upload_start = SystemTime::now();
        let output_files = self
            .upload_output_files(&exec_root, &command.output_files)
            .await?;

        let output_directories = self
            .upload_output_directories(&exec_root, &command.output_directories)
            .await?;

        // Store stdout/stderr in CAS
        let stdout_digest = if !sandbox_result.stdout.is_empty() {
            Some(self.cas_client.put(&sandbox_result.stdout).await?)
        } else {
            None
        };

        let stderr_digest = if !sandbox_result.stderr.is_empty() {
            Some(self.cas_client.put(&sandbox_result.stderr).await?)
        } else {
            None
        };

        let output_upload_completed = SystemTime::now();
        let worker_completed = SystemTime::now();

        // Clean up
        sandbox.cleanup().await?;
        let _ = tokio::fs::remove_dir_all(&exec_root).await;

        // Create result
        let result = ActionResult {
            output_files,
            output_directories,
            exit_code: sandbox_result.exit_code,
            stdout_digest,
            stderr_digest,
            execution_metadata: ExecutionMetadata {
                worker: self.config.worker_id.clone(),
                queued_timestamp,
                worker_start_timestamp: worker_start,
                worker_completed_timestamp: worker_completed,
                input_fetch_start_timestamp: input_fetch_start,
                input_fetch_completed_timestamp: input_fetch_completed,
                execution_start_timestamp: execution_start,
                execution_completed_timestamp: execution_completed,
                output_upload_start_timestamp: output_upload_start,
                output_upload_completed_timestamp: output_upload_completed,
            },
        };

        Ok(result)
    }

    /// Download inputs from CAS
    async fn download_inputs(&self, input_root_digest: &Digest, exec_root: &Path) -> Result<()> {
        // For now, we'll assume inputs are already available locally
        // In a real implementation, this would download from CAS

        // TODO: Implement proper input downloading from CAS
        // This would involve:
        // 1. Fetching the directory tree from CAS
        // 2. Creating the directory structure
        // 3. Downloading all files

        Ok(())
    }

    /// Upload output files to CAS
    async fn upload_output_files(
        &self,
        exec_root: &Path,
        output_paths: &[String],
    ) -> Result<Vec<OutputFile>> {
        let mut output_files = Vec::new();

        for path in output_paths {
            let full_path = exec_root.join(path);

            if full_path.exists() && full_path.is_file() {
                let content = tokio::fs::read(&full_path).await?;
                let digest = self.cas_client.put(&content).await?;

                let is_executable = is_executable(&full_path)?;

                output_files.push(OutputFile {
                    path: path.clone(),
                    digest,
                    is_executable,
                });

                self.stats
                    .bytes_uploaded
                    .fetch_add(content.len() as u64, Ordering::Relaxed);
            }
        }

        Ok(output_files)
    }

    /// Upload output directories to CAS
    async fn upload_output_directories(
        &self,
        exec_root: &Path,
        output_paths: &[String],
    ) -> Result<Vec<crate::remote_cache::proto::OutputDirectory>> {
        let mut output_dirs = Vec::new();

        for path in output_paths {
            let full_path = exec_root.join(path);

            if full_path.exists() && full_path.is_dir() {
                let tree_digest = self.cas_client.put_directory(&full_path).await?;

                output_dirs.push(crate::remote_cache::proto::OutputDirectory {
                    path: path.clone(),
                    tree_digest,
                });
            }
        }

        Ok(output_dirs)
    }

    /// Get execution paths based on platform
    fn get_exec_paths(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/sbin"),
        ]
    }

    /// Get statistics
    pub fn stats(&self) -> RemoteCacheStats {
        let cas_stats = self.cas_client.stats();
        let cache_stats = self.cache_client.stats();

        RemoteCacheStats {
            action_cache_hits: cache_stats.hits,
            action_cache_misses: cache_stats.misses,
            cas_hits: cas_stats.local_hits + cas_stats.remote_hits,
            cas_misses: cas_stats.misses,
            bytes_uploaded: self.stats.bytes_uploaded.load(Ordering::Relaxed),
            bytes_downloaded: cas_stats.bytes_downloaded,
            actions_executed: self.stats.actions_executed.load(Ordering::Relaxed),
            actions_cached: cache_stats.updates,
            cas_size_bytes: 0,   // TODO: Get from CAS
            cas_object_count: 0, // TODO: Get from CAS
        }
    }

    /// Clear all caches
    pub async fn clear_caches(&self) -> Result<()> {
        self.cas_client.clear_cache().await?;
        self.cache_client.clear();
        Ok(())
    }

    /// Run garbage collection
    pub async fn gc(&self) -> Result<(usize, u64)> {
        self.cas_client.gc().await
    }
}

/// Executor statistics
#[derive(Default)]
struct ExecutorStats {
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    actions_executed: AtomicU64,
    bytes_uploaded: AtomicU64,
}

/// Check if file is executable
#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)?;
    Ok(metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool> {
    Ok(false)
}

/// Builder for RemoteExecutor
pub struct RemoteExecutorBuilder {
    config: RemoteExecutorConfig,
}

impl RemoteExecutorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: RemoteExecutorConfig::default(),
        }
    }

    /// Set execution root
    pub fn exec_root(mut self, path: PathBuf) -> Self {
        self.config.exec_root = path;
        self
    }

    /// Set CAS configuration
    pub fn cas_config(mut self, config: crate::remote_cache::CASClientConfig) -> Self {
        self.config.cas_config = config;
        self
    }

    /// Set cache configuration
    pub fn cache_config(mut self, config: crate::remote_cache::CacheClientConfig) -> Self {
        self.config.cache_config = config;
        self
    }

    /// Set sandbox mode
    pub fn sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.config.sandbox_mode = mode;
        self
    }

    /// Set max concurrent executions
    pub fn max_concurrent_executions(mut self, max: usize) -> Self {
        self.config.max_concurrent_executions = max;
        self
    }

    /// Set digest function
    pub fn digest_function(mut self, func: DigestFunction) -> Self {
        self.config.digest_function = func;
        self
    }

    /// Set worker ID
    pub fn worker_id(mut self, id: String) -> Self {
        self.config.worker_id = id;
        self
    }

    /// Build the executor
    pub async fn build(self) -> Result<RemoteExecutor> {
        RemoteExecutor::new(self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_remote_executor_basic() {
        let temp_dir = TempDir::new().unwrap();

        let config = RemoteExecutorConfig {
            exec_root: temp_dir.path().join("exec"),
            cas_config: crate::remote_cache::CASClientConfig {
                cache_dir: temp_dir.path().join("cas"),
                ..Default::default()
            },
            cache_config: crate::remote_cache::CacheClientConfig {
                cache_dir: temp_dir.path().join("cache"),
                ..Default::default()
            },
            sandbox_mode: SandboxMode::None,
            ..Default::default()
        };

        let executor = RemoteExecutor::new(config).await.unwrap();

        // Execute a simple command
        let result = executor
            .execute_action(
                vec!["echo".to_string(), "hello".to_string()],
                HashMap::new(),
                temp_dir.path(),
                vec![],
                vec![],
                vec![],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout_digest.is_some());

        // Get stdout from CAS
        let stdout_data = executor
            .cas_client
            .get(&result.stdout_digest.unwrap())
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&stdout_data).trim(), "hello");

        // Check stats
        let stats = executor.stats();
        assert_eq!(stats.actions_executed, 1);
        assert_eq!(stats.action_cache_misses, 1);
    }

    #[tokio::test]
    async fn test_remote_executor_caching() {
        let temp_dir = TempDir::new().unwrap();

        let config = RemoteExecutorConfig {
            exec_root: temp_dir.path().join("exec"),
            cas_config: crate::remote_cache::CASClientConfig {
                cache_dir: temp_dir.path().join("cas"),
                ..Default::default()
            },
            cache_config: crate::remote_cache::CacheClientConfig {
                cache_dir: temp_dir.path().join("cache"),
                ..Default::default()
            },
            sandbox_mode: SandboxMode::None,
            ..Default::default()
        };

        let executor = RemoteExecutor::new(config).await.unwrap();

        // Execute command first time
        let result1 = executor
            .execute_action(
                vec!["echo".to_string(), "cached".to_string()],
                HashMap::new(),
                temp_dir.path(),
                vec![],
                vec![],
                vec![],
                None,
            )
            .await
            .unwrap();

        // Execute same command again
        let result2 = executor
            .execute_action(
                vec!["echo".to_string(), "cached".to_string()],
                HashMap::new(),
                temp_dir.path(),
                vec![],
                vec![],
                vec![],
                None,
            )
            .await
            .unwrap();

        // Should get same result
        assert_eq!(result1.exit_code, result2.exit_code);
        assert_eq!(result1.stdout_digest, result2.stdout_digest);

        // Check stats
        let stats = executor.stats();
        assert_eq!(stats.actions_executed, 1); // Only executed once
        assert_eq!(stats.action_cache_hits, 1); // Second was a cache hit
        assert_eq!(stats.action_cache_misses, 1); // First was a miss
    }

    #[tokio::test]
    async fn test_remote_executor_with_outputs() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().join("work");
        tokio::fs::create_dir_all(&working_dir).await.unwrap();

        let config = RemoteExecutorConfig {
            exec_root: temp_dir.path().join("exec"),
            cas_config: crate::remote_cache::CASClientConfig {
                cache_dir: temp_dir.path().join("cas"),
                ..Default::default()
            },
            cache_config: crate::remote_cache::CacheClientConfig {
                cache_dir: temp_dir.path().join("cache"),
                ..Default::default()
            },
            sandbox_mode: SandboxMode::None,
            ..Default::default()
        };

        let executor = RemoteExecutor::new(config).await.unwrap();

        // Execute command that creates output file
        let result = executor
            .execute_action(
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo test > output.txt".to_string(),
                ],
                HashMap::new(),
                &working_dir,
                vec![],
                vec!["output.txt".to_string()],
                vec![],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output_files.len(), 1);
        assert_eq!(result.output_files[0].path, "output.txt");

        // Verify output content in CAS
        let output_data = executor
            .cas_client
            .get(&result.output_files[0].digest)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output_data).trim(), "test");
    }
}
