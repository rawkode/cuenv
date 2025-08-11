//! Action cache implementation for deterministic task execution
//!
//! This module provides caching for task actions, including memoization
//! of results and integration with content-addressed storage.

use super::ConcurrentCache;
use crate::content_addressed_store::ContentAddressedStore;
use crate::keys::CacheKeyGenerator;
use crate::security::signing::{CacheSigner, SignedCacheEntry};
use cuenv_core::{Error, Result};
use cuenv_core::{TaskDefinition, TaskExecutionMode};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Result of a cached action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Exit code of the action
    pub exit_code: i32,
    /// Stdout content hash (stored in CAS)
    pub stdout_hash: Option<String>,
    /// Stderr content hash (stored in CAS)
    pub stderr_hash: Option<String>,
    /// Output file hashes (path -> CAS hash)
    pub output_files: HashMap<String, String>,
    /// When this action was executed
    pub executed_at: SystemTime,
    /// Duration of execution in milliseconds
    pub duration_ms: u64,
}

/// Action digest computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDigest {
    /// Hash of the action
    pub hash: String,
    /// Components that went into the hash
    pub components: ActionComponents,
}

/// Components that make up an action's identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionComponents {
    /// Task name
    pub task_name: String,
    /// Command or script
    pub command: Option<String>,
    /// Working directory
    pub working_dir: PathBuf,
    /// Environment variables that affect the action
    pub env_vars: HashMap<String, String>,
    /// Input file hashes (path -> hash)
    pub input_files: HashMap<String, String>,
    /// Task configuration hash
    pub config_hash: String,
}

/// Action cache that integrates with CAS
pub struct ActionCache {
    /// Concurrent cache for action results
    result_cache: Arc<ConcurrentCache>,
    /// Content-addressed storage for outputs
    cas: Arc<ContentAddressedStore>,
    /// In-flight actions to prevent duplicate execution
    in_flight: Arc<DashMap<String, Arc<tokio::sync::Notify>>>,
    /// Cryptographic signer for cache entries
    signer: Arc<CacheSigner>,
    /// Cache key generator with selective environment variable filtering
    key_generator: Arc<CacheKeyGenerator>,
}

impl ActionCache {
    /// Create a new action cache
    pub fn new(
        cas: Arc<ContentAddressedStore>,
        max_cache_size: u64,
        cache_dir: &Path,
    ) -> Result<Self> {
        let signer = Arc::new(CacheSigner::new(cache_dir).map_err(|e| Error::FileSystem {
            path: cache_dir.to_path_buf(),
            operation: "create cache signer".to_string(),
            source: std::io::Error::other(e.to_string()),
        })?);
        let key_generator = Arc::new(CacheKeyGenerator::new().map_err(|e| Error::FileSystem {
            path: cache_dir.to_path_buf(),
            operation: "create key generator".to_string(),
            source: std::io::Error::other(e.to_string()),
        })?);
        Ok(Self {
            result_cache: Arc::new(ConcurrentCache::new(max_cache_size)),
            cas,
            in_flight: Arc::new(DashMap::new()),
            signer,
            key_generator,
        })
    }

    /// Compute action digest for a task
    pub async fn compute_digest(
        &self,
        task_name: &str,
        task_definition: &TaskDefinition,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ActionDigest> {
        // Filter environment variables using selective filtering
        let filtered_env_vars = self.key_generator.filter_env_vars(task_name, &env_vars);

        let command = match &task_definition.execution_mode {
            TaskExecutionMode::Command { command } => Some(command.clone()),
            TaskExecutionMode::Script { content } => Some(content.clone()),
        };

        let mut components = ActionComponents {
            task_name: task_name.to_string(),
            command,
            working_dir: working_dir.to_path_buf(),
            env_vars: filtered_env_vars,
            input_files: HashMap::new(),
            config_hash: hash_task_definition(task_definition)?,
        };

        // Hash input files
        if !task_definition.inputs.is_empty() {
            for pattern in &task_definition.inputs {
                let files = crate::hashing::expand_glob_pattern(pattern, working_dir)?;
                for file in files {
                    // Use streaming hash computation for large files
                    let hash = compute_file_hash(&file).await?;
                    let relative_path = file
                        .strip_prefix(working_dir)
                        .unwrap_or(&file)
                        .to_string_lossy()
                        .to_string();
                    components.input_files.insert(relative_path, hash);
                }
            }
        }

        // Compute final digest
        let digest_hash = compute_action_hash(&components)?;

        Ok(ActionDigest {
            hash: digest_hash,
            components,
        })
    }

    /// Get cached action result from storage with signature verification
    pub fn get_cached_action_result(&self, hash: &str) -> Option<ActionResult> {
        self.result_cache.get(hash).and_then(|cached| {
            // Deserialize signed cache entry from stdout field
            if let Some(stdout_bytes) = &cached.stdout {
                if let Ok(stdout_str) = String::from_utf8(stdout_bytes.clone()) {
                    if let Ok(signed_entry) =
                        serde_json::from_str::<SignedCacheEntry<ActionResult>>(&stdout_str)
                    {
                        // Verify signature
                        match self.signer.verify(&signed_entry) {
                            Ok(true) => return Some(signed_entry.data),
                            Ok(false) => {
                                log::warn!(
                                    "Cache entry signature verification failed for hash: {hash}"
                                );
                                return None;
                            }
                            Err(e) => {
                                log::error!("Error verifying cache entry signature: {e}");
                                return None;
                            }
                        }
                    }
                }
            }

            // Fallback to legacy format for backward compatibility
            let stdout_hash = cached
                .stdout
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string());
            let stderr_hash = cached
                .stderr
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string());

            Some(ActionResult {
                exit_code: cached.exit_code,
                stdout_hash,
                stderr_hash,
                output_files: cached.output_files.clone(),
                executed_at: cached.executed_at,
                duration_ms: 0, // Not stored in CachedTaskResult
            })
        })
    }

    /// Check if an action result is cached
    pub async fn get_cached_result(&self, digest: &ActionDigest) -> Option<ActionResult> {
        // Just check cache, don't wait for in-flight actions
        // The execute_action method handles in-flight coordination
        self.get_cached_action_result(&digest.hash)
    }

    /// Execute an action with caching
    pub async fn execute_action<F, Fut>(
        &self,
        digest: &ActionDigest,
        execute_fn: F,
    ) -> Result<ActionResult>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<ActionResult>>,
    {
        // Check cache first
        if let Some(cached) = self.get_cached_result(digest).await {
            return Ok(cached);
        }

        // Try to mark as in-flight
        let notify = Arc::new(tokio::sync::Notify::new());

        // Check if we can insert (returns None if key doesn't exist)
        if let Some(existing_notify) = self.in_flight.insert(digest.hash.clone(), notify.clone()) {
            // Another task is already executing this action
            // Put the existing notify back
            self.in_flight
                .insert(digest.hash.clone(), existing_notify.clone());

            // Retry loop with timeout to handle race conditions
            let timeout = Duration::from_secs(60);
            let start = Instant::now();
            let retry_delay = Duration::from_millis(100);

            loop {
                // Check if we've exceeded the total timeout
                if start.elapsed() >= timeout {
                    return Err(Error::configuration(
                        "Timeout waiting for concurrent action execution".to_string(),
                    ));
                }

                // Try to get from cache first
                if let Some(cached) = self.get_cached_action_result(&digest.hash) {
                    return Ok(cached);
                }

                // Check if action is still in flight
                if let Some(entry) = self.in_flight.get(&digest.hash) {
                    let wait_notify = entry.value().clone();
                    drop(entry); // Release the lock

                    // Create notified future before checking cache again
                    let notified = wait_notify.notified();

                    // Double-check cache before waiting
                    if let Some(cached) = self.get_cached_action_result(&digest.hash) {
                        return Ok(cached);
                    }

                    // Wait for notification with timeout
                    let remaining_time = timeout.saturating_sub(start.elapsed());
                    match tokio::time::timeout(remaining_time, notified).await {
                        Ok(_) => {
                            // Notification received, continue to next iteration
                        }
                        Err(_) => {
                            // Timeout - check cache one more time before failing
                            if let Some(cached) = self.get_cached_action_result(&digest.hash) {
                                return Ok(cached);
                            }
                            return Err(Error::configuration(
                                "Timeout waiting for concurrent action execution".to_string(),
                            ));
                        }
                    }
                } else {
                    // Not in flight anymore, check cache with retries
                    for _ in 0..10 {
                        if let Some(cached) = self.get_cached_action_result(&digest.hash) {
                            return Ok(cached);
                        }
                        tokio::time::sleep(retry_delay).await;
                    }

                    // Action completed but result not found
                    return Err(Error::configuration(
                        "Action execution completed but result not found in cache".to_string(),
                    ));
                }
            }
        }

        // Execute the action (we already inserted ourselves into in_flight)
        let result = match execute_fn().await {
            Ok(mut result) => {
                // Store outputs in CAS
                result = self.store_outputs_in_cas(result).await?;
                result
            }
            Err(e) => {
                // Remove from in-flight and notify waiters
                self.in_flight.remove(&digest.hash);
                notify.notify_waiters();
                return Err(e);
            }
        };

        // Cache the result with cryptographic signing
        let signed_result = self
            .signer
            .sign(&result)
            .map_err(|e| Error::configuration(format!("Failed to sign cache entry: {e}")))?;

        let signed_json = serde_json::to_string(&signed_result).map_err(|e| Error::Json {
            message: "Failed to serialize signed cache entry".to_string(),
            source: e,
        })?;

        let cached_result = crate::types::CachedTaskResult {
            cache_key: digest.hash.clone(),
            executed_at: result.executed_at,
            exit_code: result.exit_code,
            stdout: Some(signed_json.as_bytes().to_vec()),
            stderr: None, // Not used in signed format
            output_files: result.output_files.clone(),
        };

        self.result_cache
            .insert(digest.hash.clone(), cached_result)?;

        // Remove from in-flight and notify waiters
        self.in_flight.remove(&digest.hash);
        notify.notify_waiters();

        Ok(result)
    }

    /// Store action outputs in CAS
    async fn store_outputs_in_cas(&self, mut result: ActionResult) -> Result<ActionResult> {
        // Store stdout if present
        if let Some(stdout_content) = result.stdout_hash.as_ref() {
            let hash = self.cas.store(Cursor::new(stdout_content.as_bytes()))?;
            result.stdout_hash = Some(hash);
        }

        // Store stderr if present
        if let Some(stderr_content) = result.stderr_hash.as_ref() {
            let hash = self.cas.store(Cursor::new(stderr_content.as_bytes()))?;
            result.stderr_hash = Some(hash);
        }

        // Store output files
        let mut new_output_files = HashMap::new();
        for (path, content_hash) in &result.output_files {
            // In a real implementation, we'd read the file and store it
            // For now, we'll assume the hash is already computed
            new_output_files.insert(path.clone(), content_hash.clone());
        }
        result.output_files = new_output_files;

        Ok(result)
    }

    /// Get statistics
    pub fn stats(&self) -> super::CacheStatSnapshot {
        self.result_cache.stats()
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.result_cache.clear();
        self.in_flight.clear();
    }
}

/// Compute hash of task definition for cache key
fn hash_task_definition(definition: &TaskDefinition) -> Result<String> {
    let serialized = serde_json::to_string(definition).map_err(|e| Error::Json {
        message: "Failed to serialize task definition for hashing".to_string(),
        source: e,
    })?;

    Ok(compute_hash(serialized.as_bytes()))
}

/// Compute hash of action components
fn compute_action_hash(components: &ActionComponents) -> Result<String> {
    let serialized = serde_json::to_string(components).map_err(|e| Error::Json {
        message: "Failed to serialize action components for hashing".to_string(),
        source: e,
    })?;

    Ok(compute_hash(serialized.as_bytes()))
}

/// Compute SHA256 hash
fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA256 hash of a file using streaming
async fn compute_file_hash(file_path: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| Error::file_system(file_path, "open file for hashing", e))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .map_err(|e| Error::file_system(file_path, "read file chunk for hashing", e))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_core::{TaskCache, TaskDefinition, TaskExecutionMode};
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_action_digest_computation() {
        let temp_dir = TempDir::new().unwrap();
        let cas =
            Arc::new(ContentAddressedStore::new(temp_dir.path().to_path_buf(), 4096).unwrap());
        let cache = ActionCache::new(cas, 0, temp_dir.path()).unwrap();

        let task_definition = TaskDefinition {
            name: "test".to_string(),
            description: Some("Test task".to_string()),
            execution_mode: TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            dependencies: vec![],
            working_directory: temp_dir.path().to_path_buf(),
            shell: "sh".to_string(),
            inputs: vec![],
            outputs: vec![],
            security: None,
            cache: TaskCache {
                enabled: true,
                key: None,
                env_filter: None,
            },
            timeout: Duration::from_secs(30),
        };

        let digest = cache
            .compute_digest("test", &task_definition, temp_dir.path(), HashMap::new())
            .await
            .unwrap();

        assert!(!digest.hash.is_empty());
        assert_eq!(digest.components.task_name, "test");
        assert_eq!(digest.components.command, Some("echo hello".to_string()));
    }

    #[tokio::test]
    async fn test_action_caching() {
        let temp_dir = TempDir::new().unwrap();
        let cas =
            Arc::new(ContentAddressedStore::new(temp_dir.path().to_path_buf(), 4096).unwrap());
        let cache = ActionCache::new(cas, 0, temp_dir.path()).unwrap();

        let task_definition = TaskDefinition {
            name: "test".to_string(),
            description: Some("Test task".to_string()),
            execution_mode: TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            dependencies: vec![],
            working_directory: temp_dir.path().to_path_buf(),
            shell: "sh".to_string(),
            inputs: vec![],
            outputs: vec![],
            security: None,
            cache: TaskCache {
                enabled: true,
                key: None,
                env_filter: None,
            },
            timeout: Duration::from_secs(30),
        };

        let digest = cache
            .compute_digest("test", &task_definition, temp_dir.path(), HashMap::new())
            .await
            .unwrap();

        // Execute action
        let result = cache
            .execute_action(&digest, || async {
                Ok(ActionResult {
                    exit_code: 0,
                    stdout_hash: Some("hello\n".to_string()),
                    stderr_hash: None,
                    output_files: HashMap::new(),
                    executed_at: SystemTime::now(),
                    duration_ms: 10,
                })
            })
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);

        // Should be cached now
        let cached = cache.get_cached_result(&digest).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().exit_code, 0);

        // Stats should show hit
        let stats = cache.stats();
        assert_eq!(stats.writes, 1);
    }

    #[tokio::test]
    async fn test_concurrent_action_execution() {
        let temp_dir = TempDir::new().unwrap();
        let cas =
            Arc::new(ContentAddressedStore::new(temp_dir.path().to_path_buf(), 4096).unwrap());
        let cache = Arc::new(ActionCache::new(cas, 0, temp_dir.path()).unwrap());

        let task_definition = TaskDefinition {
            name: "test".to_string(),
            description: Some("Test task".to_string()),
            execution_mode: TaskExecutionMode::Command {
                command: "echo hello".to_string(),
            },
            dependencies: vec![],
            working_directory: temp_dir.path().to_path_buf(),
            shell: "sh".to_string(),
            inputs: vec![],
            outputs: vec![],
            security: None,
            cache: TaskCache {
                enabled: true,
                key: None,
                env_filter: None,
            },
            timeout: Duration::from_secs(30),
        };

        let digest = cache
            .compute_digest("test", &task_definition, temp_dir.path(), HashMap::new())
            .await
            .unwrap();

        // Test with just 2 concurrent executions first
        let cache1 = cache.clone();
        let digest1 = digest.clone();
        let handle1 = tokio::spawn(async move {
            println!("Task 1: Starting execution");
            let result = cache1
                .execute_action(&digest1, || async move {
                    println!("Task 1: Actually executing");
                    // Simulate some work
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    Ok(ActionResult {
                        exit_code: 0,
                        stdout_hash: Some("hello from task 1\n".to_string()),
                        stderr_hash: None,
                        output_files: HashMap::new(),
                        executed_at: SystemTime::now(),
                        duration_ms: 100,
                    })
                })
                .await;
            println!("Task 1: Finished with result: {:?}", result.is_ok());
            result
        });

        // Give first task a head start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let cache2 = cache.clone();
        let digest2 = digest.clone();
        let handle2 = tokio::spawn(async move {
            println!("Task 2: Starting execution");
            let result = cache2
                .execute_action(&digest2, || async move {
                    println!("Task 2: Actually executing (should not happen)");
                    // This should not execute
                    Ok(ActionResult {
                        exit_code: 0,
                        stdout_hash: Some("hello from task 2\n".to_string()),
                        stderr_hash: None,
                        output_files: HashMap::new(),
                        executed_at: SystemTime::now(),
                        duration_ms: 10,
                    })
                })
                .await;
            println!("Task 2: Finished with result: {:?}", result.is_ok());
            result
        });

        // Wait for both to complete
        let result1 = handle1.await.unwrap();
        let result2 = handle2.await.unwrap();

        // Both should succeed
        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Should have same result (from cache)
        let r1 = result1.unwrap();
        let r2 = result2.unwrap();
        assert_eq!(r1.exit_code, r2.exit_code);
        assert_eq!(r1.stdout_hash, r2.stdout_hash);

        // Only one execution should have happened
        let stats = cache.stats();
        println!("Cache stats: {stats:?}");
        assert_eq!(stats.writes, 1);
    }
}
