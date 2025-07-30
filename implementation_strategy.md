# Implementation Strategy

This document outlines the detailed implementation strategy for the improved cuenv cache architecture, including specific implementation steps, code organization, and rollout plan.

## Implementation Overview

The implementation will follow a phased approach, starting with infrastructure preparation and progressing through core implementation, testing, and deployment. Each phase builds upon the previous one, ensuring a stable and reliable cache system.

## Phase 1: Infrastructure Preparation (Week 1-2)

### 1.1 Create Enhanced Configuration Types

#### Files to Create:

- `src/cache/global_config.rs` - Global cache configuration
- `src/cache/remote_config.rs` - Remote cache configuration
- `src/cache/task_cache_config.rs` - Task-specific cache configuration

#### Implementation Details:

```rust
// src/cache/global_config.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalCacheConfig {
    pub enabled: bool,
    pub default_mode: CacheMode,
    pub cache_dir: PathBuf,
    pub max_size: u64,
    pub env_include: Option<Vec<String>>,
    pub env_exclude: Option<Vec<String>>,
    pub eviction_policy: EvictionPolicy,
    pub stats_retention_days: u32,
    pub remote_cache: Option<RemoteCacheConfig>,
    pub storage: StorageConfig,
}

impl Default for GlobalCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_mode: CacheMode::ReadWrite,
            cache_dir: crate::xdg::XdgPaths::cache_dir(),
            max_size: 10 * 1024 * 1024 * 1024, // 10GB
            env_include: Some(vec![
                "PATH".to_string(),
                "HOME".to_string(),
                "USER".to_string(),
                "SHELL".to_string(),
                "LANG".to_string(),
                "CUENV_*".to_string(),
            ]),
            env_exclude: Some(vec![
                "RANDOM".to_string(),
                "TEMP".to_string(),
                "TMP".to_string(),
                "TERM".to_string(),
                "SSH_*".to_string(),
            ]),
            eviction_policy: EvictionPolicy::LRU,
            stats_retention_days: 30,
            remote_cache: None,
            storage: StorageConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LRU,
    LFU,
    FIFO,
    SizeBased,
}
```

#### Implementation Steps:

1. Create configuration structs with proper serde serialization
2. Implement Default trait for sensible defaults
3. Add validation methods for configuration values
4. Create configuration file loading and saving utilities

### 1.2 Enhance TaskConfig Structure

#### Files to Modify:

- `src/cue_parser.rs` - Add cache configuration fields

#### Implementation Details:

```rust
// src/cue_parser.rs (enhanced TaskConfig)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    // ... existing fields ...

    /// Enable build cache for this task (Bazel-style caching)
    pub cache: Option<bool>,
    /// Custom cache key - if not provided, will be derived from inputs
    #[serde(rename = "cacheKey")]
    pub cache_key: Option<String>,
    /// Cache-specific configuration
    pub cache_config: Option<TaskCacheConfig>,
    // ... existing fields ...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCacheConfig {
    /// Override global environment variable inclusion
    pub env_include: Option<Vec<String>>,
    /// Override global environment variable exclusion
    pub env_exclude: Option<Vec<String>>,
    /// Additional input patterns for this task
    pub extra_inputs: Option<Vec<String>>,
    /// Files/directories that should not affect cache key
    pub ignore_inputs: Option<Vec<String>>,
    /// Custom cache key components
    pub custom_key_components: Option<HashMap<String, String>>,
    /// Cache-specific timeout
    pub timeout: Option<u32>,
    /// Cache-specific size limits
    pub max_size: Option<u64>,
}
```

#### Implementation Steps:

1. Add new fields to TaskConfig struct
2. Update CUE schema validation
3. Add backward compatibility handling
4. Create helper methods for cache configuration access

### 1.3 Create Cache Key Generator

#### Files to Create:

- `src/cache/key_generator.rs` - Cache key generation logic
- `src/cache/env_filter.rs` - Environment variable filtering

#### Implementation Details:

```rust
// src/cache/key_generator.rs
use crate::cache::env_filter::EnvironmentFilter;
use crate::cue_parser::TaskConfig;
use crate::errors::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct CacheKeyGenerator {
    env_filter: EnvironmentFilter,
    hash_engine: Arc<HashEngine>,
}

impl CacheKeyGenerator {
    pub fn new(
        global_env_include: Option<Vec<String>>,
        global_env_exclude: Option<Vec<String>>,
    ) -> Self {
        Self {
            env_filter: EnvironmentFilter::new(global_env_include, global_env_exclude),
            hash_engine: Arc::new(HashEngine::new()),
        }
    }

    pub async fn generate_action_digest(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
        env_vars: &HashMap<String, String>,
    ) -> Result<ActionDigest> {
        let mut components = ActionComponents {
            task_name: task_name.to_string(),
            command: task_config.command.clone().or(task_config.script.clone()),
            working_dir: self.normalize_working_directory(working_dir)?,
            env_vars: self.filter_environment_variables(env_vars, task_config),
            input_files: self.hash_input_files(task_config, working_dir).await?,
            config_hash: self.hash_task_config(task_config)?,
        };

        let digest_hash = self.compute_action_hash(&components)?;

        Ok(ActionDigest {
            hash: digest_hash,
            components,
        })
    }

    fn filter_environment_variables(
        &self,
        env_vars: &HashMap<String, String>,
        task_config: &TaskConfig,
    ) -> HashMap<String, String> {
        self.env_filter.filter(env_vars, task_config)
    }

    async fn hash_input_files(
        &self,
        task_config: &TaskConfig,
        working_dir: &Path,
    ) -> Result<HashMap<String, String>> {
        let mut input_files = HashMap::new();

        // Get input patterns from task config
        let patterns = task_config.inputs.as_deref().unwrap_or(&[]);

        // Add extra inputs from cache config
        if let Some(cache_config) = &task_config.cache_config {
            if let Some(extra_inputs) = &cache_config.extra_inputs {
                patterns = &[patterns, extra_inputs.as_slice()].concat();
            }
        }

        for pattern in patterns {
            let files = crate::cache::hash_engine::expand_glob_pattern(pattern, working_dir)?;
            for file in files {
                // Skip ignored files
                if self.should_ignore_file(&file, task_config) {
                    continue;
                }

                let hash = self.hash_engine.hash_file(&file).await?;
                let relative_path = file
                    .strip_prefix(working_dir)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .to_string();
                input_files.insert(relative_path, hash);
            }
        }

        Ok(input_files)
    }

    fn should_ignore_file(&self, file_path: &Path, task_config: &TaskConfig) -> bool {
        if let Some(cache_config) = &task_config.cache_config {
            if let Some(ignore_patterns) = &cache_config.ignore_inputs {
                for pattern in ignore_patterns {
                    if let Ok(glob) = glob::Pattern::new(pattern) {
                        if glob.matches_path(file_path) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}
```

#### Implementation Steps:

1. Create environment variable filtering logic
2. Implement input file hashing with glob support
3. Add working directory normalization
4. Create task configuration hashing
5. Add comprehensive error handling

## Phase 2: Core Implementation (Week 3-4)

### 2.1 Enhanced CacheManager

#### Files to Create:

- `src/cache/enhanced_cache_manager.rs` - Enhanced cache manager
- `src/cache/cache_orchestrator.rs` - Cache orchestration logic

#### Implementation Details:

```rust
// src/cache/enhanced_cache_manager.rs
use crate::cache::action_cache::ActionCache;
use crate::cache::content_addressed_store::ContentAddressedStore;
use crate::cache::global_config::GlobalCacheConfig;
use crate::cache::key_generator::CacheKeyGenerator;
use crate::cache::remote_cache::RemoteCacheClient;
use crate::cue_parser::TaskConfig;
use crate::errors::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct EnhancedCacheManager {
    global_config: GlobalCacheConfig,
    action_cache: Arc<ActionCache>,
    content_store: Arc<ContentAddressedStore>,
    remote_cache: Option<Arc<RemoteCacheClient>>,
    key_generator: Arc<CacheKeyGenerator>,
    statistics: Arc<RwLock<CacheStatistics>>,
}

impl EnhancedCacheManager {
    pub async fn new(global_config: GlobalCacheConfig) -> Result<Self> {
        // Create cache directories
        tokio::fs::create_dir_all(&global_config.cache_dir).await?;

        let cas_dir = global_config.cache_dir.join("cas");
        let action_dir = global_config.cache_dir.join("actions");

        tokio::fs::create_dir_all(&cas_dir).await?;
        tokio::fs::create_dir_all(&action_dir).await?;

        // Initialize content-addressed store
        let content_store = Arc::new(
            ContentAddressedStore::new(cas_dir, global_config.storage.inline_threshold)?
        );

        // Initialize action cache
        let action_cache = Arc::new(ActionCache::new(
            Arc::clone(&content_store),
            global_config.max_size,
            &global_config.cache_dir,
        )?);

        // Initialize key generator
        let key_generator = Arc::new(CacheKeyGenerator::new(
            global_config.env_include.clone(),
            global_config.env_exclude.clone(),
        ));

        // Initialize remote cache if configured
        let remote_cache = if let Some(remote_config) = &global_config.remote_cache {
            Some(Arc::new(RemoteCacheClient::new(remote_config.clone()).await?))
        } else {
            None
        };

        Ok(Self {
            global_config,
            action_cache,
            content_store,
            remote_cache,
            key_generator,
            statistics: Arc::new(RwLock::new(CacheStatistics::default())),
        })
    }

    pub fn is_caching_enabled(&self, task_config: &TaskConfig) -> bool {
        if !self.global_config.enabled {
            return false;
        }

        task_config.cache.unwrap_or(true)
    }

    pub fn get_task_cache_mode(&self, task_config: &TaskConfig) -> CacheMode {
        // For now, use global default - could be extended with per-task mode
        self.global_config.default_mode
    }

    pub async fn execute_task_with_cache<F, Fut>(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
        env_vars: &HashMap<String, String>,
        execute_fn: F,
    ) -> Result<ActionResult>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<ActionResult>>,
    {
        // Check if caching is enabled for this task
        if !self.is_caching_enabled(task_config) {
            log::debug!("Cache disabled for task '{}', executing directly", task_name);
            return execute_fn().await;
        }

        // Generate cache key
        let digest = self
            .key_generator
            .generate_action_digest(task_name, task_config, working_dir, env_vars)
            .await?;

        log::debug!("Generated cache key for task '{}': {}", task_name, digest.hash);

        // Execute with caching
        let result = self.action_cache.execute_action(&digest, execute_fn).await?;

        // Update statistics
        {
            let mut stats = self.statistics.write().unwrap();
            stats.writes += 1;
        }

        // Upload to remote cache if configured
        if let Some(remote_cache) = &self.remote_cache {
            if self.global_config.remote_cache.as_ref().unwrap().upload_enabled {
                if let Err(e) = remote_cache.upload_action_result(&digest, &result).await {
                    log::warn!("Failed to upload result to remote cache: {}", e);
                }
            }
        }

        Ok(result)
    }

    pub async fn get_cached_result(&self, digest: &ActionDigest) -> Option<ActionResult> {
        // Check local cache first
        if let Some(result) = self.action_cache.get_cached_result(digest).await {
            return Some(result);
        }

        // Check remote cache if configured
        if let Some(remote_cache) = &self.remote_cache {
            if self.global_config.remote_cache.as_ref().unwrap().download_enabled {
                if let Ok(result) = remote_cache.get_action_result(digest).await {
                    // Store remote result in local cache
                    if let Err(e) = self.action_cache.store_remote_result(digest, &result).await {
                        log::warn!("Failed to store remote result in local cache: {}", e);
                    }
                    return Some(result);
                }
            }
        }

        None
    }
}
```

#### Implementation Steps:

1. Create enhanced cache manager with proper ActionCache integration
2. Implement persistent storage delegation
3. Add remote cache integration
4. Create cache statistics tracking
5. Add comprehensive error handling and logging

### 2.2 TaskExecutor Integration

#### Files to Modify:

- `src/task_executor.rs` - Integrate enhanced cache manager

#### Implementation Details:

```rust
// src/task_executor.rs (enhanced execute_single_task_with_cache)
impl TaskExecutor {
    async fn execute_single_task_with_cache(
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
        args: &[String],
        cache_manager: &Arc<EnhancedCacheManager>,
        audit_mode: bool,
    ) -> Result<i32> {
        // Check if caching is enabled for this task
        if !cache_manager.is_caching_enabled(task_config) {
            log::info!("Cache disabled for task '{}', executing directly", task_name);
            return self.execute_task_directly(task_name, task_config, working_dir, args, audit_mode)
                .await;
        }

        // Get current environment variables
        let env_vars: HashMap<String, String> = std::env::vars().collect();

        // Execute task with caching
        let result = cache_manager
            .execute_task_with_cache(
                task_name,
                task_config,
                working_dir,
                &env_vars,
                || async {
                    self.execute_task_directly(task_name, task_config, working_dir, args, audit_mode)
                        .await
                        .map(|exit_code| ActionResult {
                            exit_code,
                            stdout_hash: None, // Will be filled by cache system
                            stderr_hash: None,
                            output_files: HashMap::new(),
                            executed_at: SystemTime::now(),
                            duration_ms: 0, // Will be calculated
                        })
                },
            )
            .await?;

        Ok(result.exit_code)
    }

    async fn execute_task_directly(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        // Existing task execution logic
        // ... (current implementation)
    }
}
```

#### Implementation Steps:

1. Modify TaskExecutor to use EnhancedCacheManager
2. Add cache checking logic before task execution
3. Integrate with existing security and audit features
4. Maintain backward compatibility with existing APIs

### 2.3 Remote Cache Client

#### Files to Create:

- `src/cache/remote_cache.rs` - Remote cache client implementation

#### Implementation Details:

```rust
// src/cache/remote_cache.rs
use crate::cache::action_cache::{ActionDigest, ActionResult};
use crate::cache::remote_config::RemoteCacheConfig;
use crate::errors::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct RemoteCacheClient {
    config: RemoteCacheConfig,
    grpc_client: Arc<CacheServiceClient<Channel>>,
    request_semaphore: Arc<Semaphore>,
}

impl RemoteCacheClient {
    pub async fn new(config: RemoteCacheConfig) -> Result<Self> {
        let endpoint = Channel::from_shared(config.endpoint.clone())?;
        let grpc_client = CacheServiceClient::connect(endpoint)
            .await
            .map_err(|e| Error::configuration(format!("Failed to connect to remote cache: {}", e)))?;

        Ok(Self {
            config,
            grpc_client: Arc::new(grpc_client),
            request_semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
        })
    }

    pub async fn has_action_result(&self, digest: &ActionDigest) -> Result<bool> {
        let _permit = self.request_semaphore.acquire().await?;

        let request = tonic::Request::new(HasActionResultRequest {
            digest: Some(digest.clone().into()),
        });

        let response = self
            .grpc_client
            .clone()
            .has_action_result(request)
            .await
            .map_err(|e| Error::configuration(format!("Remote cache error: {}", e)))?;

        Ok(response.into_inner().has_result)
    }

    pub async fn get_action_result(&self, digest: &ActionDigest) -> Result<ActionResult> {
        let _permit = self.request_semaphore.acquire().await?;

        let request = tonic::Request::new(GetActionResultRequest {
            digest: Some(digest.clone().into()),
        });

        let response = self
            .grpc_client
            .clone()
            .get_action_result(request)
            .await
            .map_err(|e| Error::configuration(format!("Remote cache error: {}", e)))?;

        let inner = response.into_inner();
        inner
            .result
            .ok_or_else(|| Error::configuration("Action result not found in remote cache"))
            .map(|r| r.into())
    }

    pub async fn upload_action_result(
        &self,
        digest: &ActionDigest,
        result: &ActionResult,
    ) -> Result<()> {
        let _permit = self.request_semaphore.acquire().await?;

        let request = tonic::Request::new(UpdateActionResultRequest {
            digest: Some(digest.clone().into()),
            result: Some(result.clone().into()),
        });

        self.grpc_client
            .clone()
            .update_action_result(request)
            .await
            .map_err(|e| Error::configuration(format!("Remote cache error: {}", e)))?;

        Ok(())
    }
}
```

#### Implementation Steps:

1. Implement gRPC client for remote cache
2. Add authentication and error handling
3. Implement concurrency control with semaphore
4. Add retry logic for network failures
5. Implement proper connection management

## Phase 3: Testing and Validation (Week 5)

### 3.1 Unit Tests

#### Files to Create:

- `tests/cache_config_tests.rs` - Configuration tests
- `tests/cache_key_tests.rs` - Cache key generation tests
- `tests/remote_cache_tests.rs` - Remote cache tests

#### Implementation Details:

```rust
// tests/cache_config_tests.rs
use cuenv::cache::global_config::GlobalCacheConfig;
use cuenv::cache::remote_config::RemoteCacheConfig;
use tempfile::TempDir;

#[test]
fn test_default_config() {
    let config = GlobalCacheConfig::default();
    assert!(config.enabled);
    assert_eq!(config.default_mode, CacheMode::ReadWrite);
    assert!(config.max_size > 0);
}

#[test]
fn test_config_validation() {
    let mut config = GlobalCacheConfig::default();
    config.max_size = 0; // Invalid size

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_env_filtering() {
    let config = GlobalCacheConfig {
        env_include: Some(vec!["PATH".to_string(), "HOME".to_string()]),
        env_exclude: Some(vec!["TEMP".to_string()]),
        ..Default::default()
    };

    let env_vars = vec![
        ("PATH", "/usr/bin:/bin"),
        ("HOME", "/home/user"),
        ("TEMP", "/tmp"),
        ("RANDOM", "12345"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    let filtered = config.filter_environment_variables(&env_vars);
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains_key("PATH"));
    assert!(filtered.contains_key("HOME"));
    assert!(!filtered.contains_key("TEMP"));
    assert!(!filtered.contains_key("RANDOM"));
}

#[tokio::test]
async fn test_remote_cache_config() {
    let config = RemoteCacheConfig {
        endpoint: "grpc://localhost:9092".to_string(),
        auth_token: Some("test-token".to_string()),
        timeout_seconds: 30,
        max_concurrent: 5,
        upload_enabled: true,
        download_enabled: true,
    };

    let result = RemoteCacheClient::new(config).await;
    // This will fail to connect, but should validate config
    assert!(result.is_err());
}
```

#### Implementation Steps:

1. Create comprehensive unit tests for all new components
2. Test configuration parsing and validation
3. Test environment variable filtering
4. Test cache key generation with various inputs
5. Test remote cache client with mock server

### 3.2 Integration Tests

#### Files to Create:

- `tests/cache_integration_tests.rs` - End-to-end cache tests
- `tests/concurrent_cache_tests.rs` - Concurrency tests

#### Implementation Details:

```rust
// tests/cache_integration_tests.rs
use cuenv::cache::enhanced_cache_manager::EnhancedCacheManager;
use cuenv::cache::global_config::GlobalCacheConfig;
use cuenv::cue_parser::TaskConfig;
use tempfile::TempDir;
use std::collections::HashMap;

#[tokio::test]
async fn test_task_caching_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config = GlobalCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let cache_manager = EnhancedCacheManager::new(config).await.unwrap();

    let task_config = TaskConfig {
        description: Some("Test task".to_string()),
        command: Some("echo 'test'".to_string()),
        cache: Some(true),
        ..Default::default()
    };

    let working_dir = temp_dir.path();
    let env_vars = HashMap::new();

    // Execute task first time
    let result1 = cache_manager
        .execute_task_with_cache(
            "test",
            &task_config,
            working_dir,
            &env_vars,
            || async {
                Ok(ActionResult {
                    exit_code: 0,
                    stdout_hash: Some("test\n".to_string()),
                    stderr_hash: None,
                    output_files: HashMap::new(),
                    executed_at: SystemTime::now(),
                    duration_ms: 10,
                })
            },
        )
        .await
        .unwrap();

    assert_eq!(result1.exit_code, 0);

    // Execute task second time (should hit cache)
    let result2 = cache_manager
        .execute_task_with_cache(
            "test",
            &task_config,
            working_dir,
            &env_vars,
            || async {
                panic!("This should not be called due to cache hit");
            },
        )
        .await
        .unwrap();

    assert_eq!(result2.exit_code, 0);
}

#[tokio::test]
async fn test_cache_disabled_task() {
    let temp_dir = TempDir::new().unwrap();
    let config = GlobalCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let cache_manager = EnhancedCacheManager::new(config).await.unwrap();

    let task_config = TaskConfig {
        description: Some("Test task".to_string()),
        command: Some("echo 'test'".to_string()),
        cache: Some(false), // Cache disabled
        ..Default::default()
    };

    let working_dir = temp_dir.path();
    let env_vars = HashMap::new();

    let mut execution_count = 0;

    // Execute task - should always run directly
    let result = cache_manager
        .execute_task_with_cache(
            "test",
            &task_config,
            working_dir,
            &env_vars,
            || async {
                execution_count += 1;
                Ok(ActionResult {
                    exit_code: 0,
                    stdout_hash: Some("test\n".to_string()),
                    stderr_hash: None,
                    output_files: HashMap::new(),
                    executed_at: SystemTime::now(),
                    duration_ms: 10,
                })
            },
        )
        .await
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(execution_count, 1);

    // Execute again - should still run directly
    let result2 = cache_manager
        .execute_task_with_cache(
            "test",
            &task_config,
            working_dir,
            &env_vars,
            || async {
                execution_count += 1;
                Ok(ActionResult {
                    exit_code: 0,
                    stdout_hash: Some("test\n".to_string()),
                    stderr_hash: None,
                    output_files: HashMap::new(),
                    executed_at: SystemTime::now(),
                    duration_ms: 10,
                })
            },
        )
        .await
        .unwrap();

    assert_eq!(result2.exit_code, 0);
    assert_eq!(execution_count, 2);
}
```

#### Implementation Steps:

1. Create end-to-end integration tests
2. Test cache hit/miss scenarios
3. Test concurrent execution scenarios
4. Test remote cache integration
5. Test cache persistence across invocations

### 3.3 Performance Tests

#### Files to Create:

- `benches/cache_performance.rs` - Performance benchmarks

#### Implementation Details:

```rust
// benches/cache_performance.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use cuenv::cache::enhanced_cache_manager::EnhancedCacheManager;
use cuenv::cache::global_config::GlobalCacheConfig;
use cuenv::cue_parser::TaskConfig;
use tempfile::TempDir;

fn bench_cache_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_generation");

    for input_count in [0, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("input_files", input_count),
            input_count,
            |b, &count| {
                let temp_dir = TempDir::new().unwrap();
                let config = GlobalCacheConfig::default();
                let cache_manager = EnhancedCacheManager::new(config).await.unwrap();

                // Create test files
                for i in 0..count {
                    let file_path = temp_dir.path().join(format!("input_{}.txt", i));
                    tokio::fs::write(&file_path, format!("content {}", i)).await.unwrap();
                }

                let task_config = TaskConfig {
                    inputs: Some((0..count).map(|i| format!("input_{}.txt", i)).collect()),
                    ..Default::default()
                };

                b.to_async(&tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        cache_manager
                            .key_generator
                            .generate_action_digest(
                                "test",
                                &task_config,
                                temp_dir.path(),
                                &std::env::vars().collect(),
                            )
                            .await
                    });
            },
        );
    }

    group.finish();
}

fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_operations");

    let temp_dir = TempDir::new().unwrap();
    let config = GlobalCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let cache_manager = EnhancedCacheManager::new(config).await.unwrap();
    let task_config = TaskConfig {
        cache: Some(true),
        ..Default::default()
    };

    group.bench_function("cache_hit", |b| {
        b.to_async(&tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                cache_manager
                    .execute_task_with_cache(
                        "test",
                        &task_config,
                        temp_dir.path(),
                        &std::env::vars().collect(),
                        || async {
                            Ok(ActionResult {
                                exit_code: 0,
                                stdout_hash: Some("cached_result".to_string()),
                                stderr_hash: None,
                                output_files: HashMap::new(),
                                executed_at: SystemTime::now(),
                                duration_ms: 1,
                            })
                        },
                    )
                    .await
            });
    });

    group.finish();
}

criterion_group!(benches, bench_cache_key_generation, bench_cache_operations);
criterion_main!(benches);
```

#### Implementation Steps:

1. Create performance benchmarks for cache key generation
2. Benchmark cache hit/miss operations
3. Test with various input sizes
4. Measure memory usage and CPU utilization
5. Optimize based on benchmark results

## Phase 4: Migration and Deployment (Week 6)

### 4.1 Backward Compatibility

#### Files to Modify:

- `src/cache/cache_manager.rs` - Add compatibility layer
- `src/lib.rs` - Export both old and new APIs

#### Implementation Details:

```rust
// src/cache/cache_manager.rs (compatibility layer)
use crate::cache::enhanced_cache_manager::EnhancedCacheManager;
use crate::cache::global_config::GlobalCacheConfig;

/// Legacy cache manager for backward compatibility
pub struct CacheManager {
    enhanced: EnhancedCacheManager,
}

impl CacheManager {
    /// Create a new cache manager (legacy API)
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let global_config = GlobalCacheConfig {
            enabled: config.mode != CacheMode::Off,
            default_mode: config.mode,
            cache_dir: config.base_dir,
            max_size: config.max_size,
            ..Default::default()
        };

        let enhanced = EnhancedCacheManager::new(global_config).await?;
        Ok(Self { enhanced })
    }

    /// Create a new cache manager (sync version for main application)
    pub fn new_sync() -> Result<Self> {
        let global_config = GlobalCacheConfig::default();
        let enhanced = crate::async_runtime::run_async(EnhancedCacheManager::new(global_config))?;
        Ok(Self { enhanced })
    }

    /// Legacy API for backward compatibility
    pub fn get_cached_result(&self, cache_key: &str) -> Option<CachedTaskResult> {
        // Convert legacy cache key to new format and check cache
        // This is a simplified implementation
        None
    }

    /// Legacy API for backward compatibility
    pub fn store_result(&self, cache_key: String, result: CachedTaskResult) -> Result<()> {
        // Convert legacy result to new format and store
        // This is a simplified implementation
        Ok(())
    }

    /// Get enhanced cache manager for new features
    pub fn enhanced(&self) -> &EnhancedCacheManager {
        &self.enhanced
    }
}
```

#### Implementation Steps:

1. Create compatibility layer for existing APIs
2. Add deprecation warnings for old APIs
3. Provide migration guide for users
4. Ensure existing tests continue to pass

### 4.2 Documentation and Examples

#### Files to Create:

- `docs/cache_migration_guide.md` - Migration guide
- `examples/cache_examples/` - Example configurations

#### Implementation Details:

````markdown
# Cache Migration Guide

## Overview

The new cache system provides enhanced functionality while maintaining backward compatibility with existing configurations.

## What's New

### 1. Global Cache Configuration

- Configure cache behavior globally with JSON files
- Environment variable overrides
- Remote cache integration

### 2. Enhanced Task Configuration

- Per-task cache configuration
- Selective environment variable inclusion
- Custom cache key components

### 3. Improved Performance

- Persistent disk-based storage
- Better cache key generation
- Remote cache support

## Migration Steps

### Step 1: Update Configuration (Optional)

Create a global cache configuration file:

```json
{
	"enabled": true,
	"default_mode": "read-write",
	"cache_dir": "~/.cache/cuenv",
	"max_size": 10737418240
}
```
````

### Step 2: Update Task Configuration (Optional)

Enhance task configurations with new cache options:

```cue
tasks: {
  "build": {
    description: "Build the project"
    command: "make build"
    cache: true  // This still works

    // New enhanced configuration
    cacheConfig: {
      envInclude: ["PATH", "HOME", "RUST*"]
      extraInputs: ["config/**"]
      ignoreInputs: ["*.tmp"]
    }
  }
}
```

### Step 3: Update Code (Optional)

If you use the cache API directly, consider using the enhanced version:

```rust
// Old way (still works)
let cache_manager = CacheManager::new_sync()?;

// New way with enhanced features
let global_config = GlobalCacheConfig::default();
let cache_manager = EnhancedCacheManager::new(global_config).await?;
```

## Backward Compatibility

All existing configurations continue to work without changes. The new features are opt-in.

## Testing Your Migration

1. **Validate Configuration**

   ```bash
   cuenv cache validate
   ```

2. **Test Cache Operations**

   ```bash
   cuenv cache test
   ```

3. **Monitor Performance**
   ```bash
   cuenv cache stats
   ```

## Troubleshooting

### Common Issues

1. **Cache Not Working**

   - Check if caching is enabled globally
   - Verify task configuration
   - Check cache directory permissions

2. **Performance Issues**

   - Monitor cache hit rates
   - Check disk space usage
   - Verify network connectivity for remote cache

3. **Configuration Errors**
   - Validate configuration files
   - Check environment variable syntax
   - Verify file paths and permissions

## Getting Help

- Documentation: [Cache Configuration](cache_configuration.md)
- Examples: [Cache Examples](../examples/cache_examples/)
- Issues: [GitHub Issues](https://github.com/rawkode/cuenv/issues)

````

#### Implementation Steps:
1. Create comprehensive migration guide
2. Add example configurations
3. Update user documentation
4. Create troubleshooting guide

### 4.3 Rollout Strategy

#### Implementation Details:

```rust
// src/cache/feature_flags.rs
pub struct CacheFeatureFlags {
    pub enhanced_cache_enabled: bool,
    pub remote_cache_enabled: bool,
    pub persistent_storage_enabled: bool,
}

impl CacheFeatureFlags {
    pub fn from_env() -> Self {
        Self {
            enhanced_cache_enabled: std::env::var("CUENV_ENHANCED_CACHE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true),
            remote_cache_enabled: std::env::var("CUENV_REMOTE_CACHE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true),
            persistent_storage_enabled: std::env::var("CUENV_PERSISTENT_CACHE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true),
        }
    }
}

// Integration with cache manager
impl EnhancedCacheManager {
    pub async fn new_with_flags(
        global_config: GlobalCacheConfig,
        flags: CacheFeatureFlags,
    ) -> Result<Self> {
        let mut manager = Self::new(global_config).await?;

        if !flags.enhanced_cache_enabled {
            // Fall back to legacy behavior
            manager.use_legacy_mode = true;
        }

        if !flags.remote_cache_enabled {
            manager.remote_cache = None;
        }

        if !flags.persistent_storage_enabled {
            manager.use_memory_only = true;
        }

        Ok(manager)
    }
}
````

#### Implementation Steps:

1. Implement feature flags for gradual rollout
2. Add monitoring and telemetry
3. Create rollback procedures
4. Establish success metrics

## Success Metrics

### Technical Metrics

- **Cache Hit Rate**: Target > 80% for deterministic tasks
- **Cache Key Generation Time**: < 100ms for typical projects
- **Cache Storage Overhead**: < 5% of original task execution time
- **Memory Usage**: < 100MB additional memory usage
- **Disk Usage**: Configurable with sensible defaults

### User Experience Metrics

- **Configuration Complexity**: Minimal changes required for existing users
- **Performance Improvement**: Noticeable speedup for repeated tasks
- **Error Rate**: < 1% cache-related errors
- **Documentation Coverage**: Complete documentation for all features

### Business Metrics

- **Development Time Reduction**: Target 20-30% reduction in build/test time
- **Resource Usage**: Optimal use of disk space and network bandwidth
- **User Satisfaction**: Positive feedback from early adopters
- **Adoption Rate**: Gradual increase in feature usage

## Risk Mitigation

### Technical Risks

1. **Performance Regression**

   - Comprehensive benchmarking
   - Performance monitoring in production
   - Quick rollback capability

2. **Data Loss**

   - Backup procedures for cache data
   - Integrity checking and validation
   - Graceful degradation on failures

3. **Compatibility Issues**
   - Extensive testing with existing configurations
   - Backward compatibility layer
   - Clear migration path

### Operational Risks

1. **Deployment Issues**

   - Phased rollout with monitoring
   - Feature flags for quick disable
   - Rollback procedures

2. **User Adoption**
   - Clear documentation and examples
   - Migration tools and assistance
   - Community support and feedback

## Conclusion

This implementation strategy provides a comprehensive approach to delivering the enhanced cache system while maintaining stability and backward compatibility. The phased approach allows for careful testing and validation at each stage, ensuring a reliable and performant cache system that meets the needs of cuenv users.

The key to success is maintaining the balance between innovation and stability, ensuring that new features enhance the user experience without disrupting existing workflows.
