//! Unified cache manager with security and remote cache support
use super::{
    ActionCache, CacheConfig, CacheEngine, CacheKeyFilterConfig, CachedTaskResult,
    ContentAddressedStore,
};
use crate::async_runtime::{run_async, AsyncRuntime};
use crate::atomic_file::write_atomic_string;
use crate::cache::key_generator::CacheKeyGenerator;
use crate::cache::signing::CacheSigner;
use crate::core::errors::{Error, Result};
use crate::cue_parser::TaskConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

/// Cache version for migration support
const CACHE_VERSION: u32 = 1;

/// Statistics for cache operations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub errors: u64,
    pub lock_contentions: u64,
    pub total_bytes_saved: u64,
    pub last_cleanup: Option<SystemTime>,
}

/// Unified cache manager that provides access to cache components
pub struct CacheManager {
    config: CacheConfig,
    content_store: Arc<ContentAddressedStore>,
    action_cache: Arc<ActionCache>,
    /// Underlying cache engine for legacy compatibility
    #[allow(dead_code)]
    engine: Arc<CacheEngine>,
    /// Statistics for monitoring
    stats: Arc<RwLock<CacheStatistics>>,
    /// Cache version for migration support
    version: u32,
    /// Cache signer for integrity protection
    signer: Arc<CacheSigner>,
    /// Simple in-memory cache for task results
    memory_cache: Arc<Mutex<HashMap<String, CachedTaskResult>>>,
    /// Cache key generator with selective environment variable filtering
    key_generator: Arc<CacheKeyGenerator>,
}

impl CacheManager {
    /// Create a new cache manager with given configuration (async version)
    pub async fn new(config: CacheConfig) -> Result<Self> {
        Self::new_internal(config).await
    }

    /// Create a new cache manager (sync version for main application)
    pub fn new_sync() -> Result<Self> {
        let engine = Arc::new(CacheEngine::new()?);
        let config = CacheConfig {
            base_dir: engine.cache_dir.clone(),
            max_size: 1024 * 1024 * 1024, // 1GB default
            mode: super::CacheMode::ReadWrite,
            inline_threshold: 4096, // 4KB default
            env_filter: Default::default(),
            task_env_filters: HashMap::new(),
        };

        if AsyncRuntime::is_in_async_context() {
            return Err(Error::configuration(
                "Cannot use sync constructor from async context. Use new() instead.".to_string(),
            ));
        }

        run_async(Self::new_internal(config))
    }

    /// Internal constructor shared by both sync and async versions
    async fn new_internal(config: CacheConfig) -> Result<Self> {
        // Create cache directories
        std::fs::create_dir_all(&config.base_dir)?;

        let cas_dir = config.base_dir.join("cas");
        let action_dir = config.base_dir.join("actions");

        std::fs::create_dir_all(&cas_dir)?;
        std::fs::create_dir_all(&action_dir)?;

        // Initialize content-addressed store
        let content_store = Arc::new(ContentAddressedStore::new(
            cas_dir,
            config.inline_threshold,
        )?);

        // Initialize action cache with CAS and max size
        let action_cache = Arc::new(ActionCache::new(
            Arc::clone(&content_store),
            config.max_size,
            &config.base_dir,
        )?);

        // Initialize cache engine for legacy compatibility
        let engine = Arc::new(CacheEngine::new().map_err(|e| Error::Configuration {
            message: format!("Failed to initialize cache engine: {}", e),
        })?);
        let stats = Arc::new(RwLock::new(CacheStatistics::default()));
        let signer =
            Arc::new(
<<<<<<< HEAD
                CacheSigner::new(&config.base_dir).map_err(|e| Error::FileSystem {
                    path: config.base_dir.clone(),
                    operation: "create cache signer".to_string(),
                    source: std::io::Error::other(format!("Failed to create cache signer: {e}")),
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
        let signer = Arc::new(CacheSigner::new(&config.base_dir)?);
=======
                CacheSigner::new(&config.base_dir).map_err(|e| Error::Configuration {
                    message: format!("Failed to initialize cache signer: {}", e),
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
                })?,
            );

        // Initialize cache key generator with configuration
        let mut key_generator = CacheKeyGenerator::with_config(config.env_filter.clone())?;

        // Add task-specific configurations
        for (task_name, task_config) in &config.task_env_filters {
            key_generator.add_task_config(task_name, task_config.clone())?;
        }

        let key_generator = Arc::new(key_generator);

        let manager = Self {
            config,
            content_store,
            action_cache,
            engine,
            stats,
            version: CACHE_VERSION,
            signer,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
            key_generator,
        };

        // Check and migrate cache if needed
        manager.check_and_migrate()?;

        Ok(manager)
    }

    /// Check cache version and migrate if necessary
    fn check_and_migrate(&self) -> Result<()> {
        let version_file = self.config.base_dir.join("VERSION");

        if version_file.exists() {
            let content = fs::read_to_string(&version_file)
                .map_err(|e| Error::file_system(&version_file, "read version file", e))?;

            let file_version: u32 = content
                .trim()
                .parse()
                .map_err(|_| Error::configuration("Invalid cache version format".to_string()))?;

            if file_version < self.version {
                log::info!(
                    "Migrating cache from version {} to {}",
                    file_version,
                    self.version
                );
                self.migrate_cache(file_version)?;
            } else if file_version > self.version {
                return Err(Error::configuration(format!(
                    "Cache version {} is newer than supported version {}",
                    file_version, self.version
                )));
            }
        } else {
            // Write current version atomically
            write_atomic_string(&version_file, &self.version.to_string())?;
        }

        Ok(())
    }

    /// Migrate cache from older version
    fn migrate_cache(&self, _from_version: u32) -> Result<()> {
        // For now, just clear the cache on migration
        log::warn!("Cache migration: clearing cache due to version change");
        self.clear_cache()?;

        // Write new version
        let version_file = self.config.base_dir.join("VERSION");
        write_atomic_string(&version_file, &self.version.to_string())?;

        Ok(())
    }

    /// Clear all cache entries
    pub fn clear_cache(&self) -> Result<()> {
        // Clear action cache
        self.action_cache.clear();

        // Clear content store would require more complex logic
        log::info!("Cache cleared");

        Ok(())
    }

    /// Get the content-addressed store
    pub fn content_store(&self) -> Arc<ContentAddressedStore> {
        Arc::clone(&self.content_store)
    }

    /// Get the action cache
    pub fn action_cache(&self) -> Arc<ActionCache> {
        Arc::clone(&self.action_cache)
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Get cache statistics
    pub fn get_statistics(&self) -> CacheStatistics {
        self.stats.read().unwrap().clone()
    }

    /// Get cached result for a task
    pub fn get_cached_result(&self, cache_key: &str) -> Option<CachedTaskResult> {
        // First try to get from ActionCache
        if let Some(action_result) = self.action_cache.get_cached_action_result(cache_key) {
            // Convert ActionResult back to CachedTaskResult for backward compatibility
            let cached_result = CachedTaskResult {
                cache_key: cache_key.to_string(),
                executed_at: action_result.executed_at,
                exit_code: action_result.exit_code,
                stdout: action_result.stdout_hash.map(|s| s.as_bytes().to_vec()),
                stderr: action_result.stderr_hash.map(|s| s.as_bytes().to_vec()),
                output_files: action_result.output_files,
            };

            // Only return successful results (exit_code == 0)
            if cached_result.exit_code == 0 {
                let mut stats = self.stats.write().unwrap();
                stats.hits += 1;
                return Some(cached_result);
            }
        }

        // Fallback to in-memory cache for backward compatibility
        if let Ok(cache) = self.memory_cache.lock() {
            if let Some(result) = cache.get(cache_key) {
                // Only return successful results (exit_code == 0)
                if result.exit_code == 0 {
                    let mut stats = self.stats.write().unwrap();
                    stats.hits += 1;
                    return Some(result.clone());
                }
            }
        }

        let mut stats = self.stats.write().unwrap();
        stats.misses += 1;
        None
    }

    /// Store a cached result
    pub fn store_result(&self, cache_key: String, result: CachedTaskResult) -> Result<()> {
        // Sign the result for integrity
<<<<<<< HEAD
        let _signed_result = self.signer.sign(&result).map_err(|e| Error::FileSystem {
            path: PathBuf::from("cache"),
            operation: "sign cache result".to_string(),
            source: std::io::Error::other(e.to_string()),
        })?;
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
        let _signed_result = self.signer.sign(&result)?;
=======
        let _signed_result = self
            .signer
            .sign(&result)
            .map_err(|e| Error::Configuration {
                message: format!("Failed to sign cache result: {}", e),
            })?;
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)

        // Only cache successful results (exit_code == 0)
        if result.exit_code == 0 {
            // Store in ActionCache (this will be handled by ActionCache::execute_action)
            // For backward compatibility, also store in memory cache
            if let Ok(mut cache) = self.memory_cache.lock() {
                cache.insert(cache_key.clone(), result.clone());
            }
        }

        let mut stats = self.stats.write().unwrap();
        stats.writes += 1;

        Ok(())
    }

    /// Generate cache key for a task
    pub fn generate_cache_key(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        env_vars: &HashMap<String, String>,
        working_dir: &Path,
    ) -> Result<String> {
        // Use the selective cache key generator for improved cache hit rates
        let config_hash = hash_task_config(task_config)?;
        let command = task_config.command.as_ref().or(task_config.script.as_ref());

        // For now, use empty input files since we don't have them in this context
        let input_files = HashMap::new();

        self.key_generator.generate_cache_key(
            task_name,
            &config_hash,
            working_dir,
            &input_files,
            env_vars,
            command.map(|s| s.as_str()),
        )
    }

    /// Legacy API for backward compatibility with tests
    pub fn generate_cache_key_legacy(
        &self,
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
    ) -> Result<String> {
        // Use empty env vars for deterministic testing
        let env_vars = HashMap::new();
        self.generate_cache_key(task_name, task_config, &env_vars, working_dir)
    }

    /// Legacy API for backward compatibility with tests
    pub fn save_result(
        &self,
        cache_key: &str,
        _task_config: &TaskConfig,
        _working_dir: &Path,
        exit_code: i32,
    ) -> Result<()> {
        let result = CachedTaskResult {
            cache_key: cache_key.to_string(),
            executed_at: SystemTime::now(),
            exit_code,
            stdout: None,
            stderr: None,
            output_files: std::collections::HashMap::new(),
        };
        self.store_result(cache_key.to_string(), result)
    }

    /// Legacy API for backward compatibility with tests
    pub fn get_cached_result_legacy(
        &self,
        cache_key: &str,
        _task_config: &TaskConfig,
        _working_dir: &Path,
    ) -> Result<Option<CachedTaskResult>> {
        Ok(self.get_cached_result(cache_key))
    }

    /// Cleanup stale cache entries
    pub fn cleanup_stale_entries(&self) -> Result<()> {
        // Run garbage collection on content store
        let (removed_count, removed_bytes) = self.content_store.garbage_collect()?;

        log::info!("Cache cleanup: removed {removed_count} entries, freed {removed_bytes} bytes");

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.last_cleanup = Some(SystemTime::now());
        }

        Ok(())
    }

    /// Get the cache key generator for advanced configuration
    pub fn key_generator(&self) -> Arc<CacheKeyGenerator> {
        Arc::clone(&self.key_generator)
    }

    /// Apply task-specific cache environment configurations
    pub fn apply_task_configs(&mut self, tasks: &HashMap<String, TaskConfig>) -> Result<()> {
        // Create a new key generator with the current global config
        let mut new_key_generator = CacheKeyGenerator::with_config(self.config.env_filter.clone())?;

        // Process each task to extract cache environment configurations
        for (task_name, task_config) in tasks {
            if let Some(cache_env) = &task_config.cache_env {
                // Convert CacheEnvConfig to CacheKeyFilterConfig
                let filter_config: CacheKeyFilterConfig = cache_env.clone().into();

                // Add task-specific configuration
                new_key_generator.add_task_config(task_name, filter_config.clone())?;

                // Also update the task_env_filters in the config for persistence
                self.config
                    .task_env_filters
                    .insert(task_name.clone(), filter_config);
            }
        }

        // Replace the key generator with the updated one
        self.key_generator = Arc::new(new_key_generator);

        Ok(())
    }
}

/// Compute hash of task configuration for cache key generation
fn hash_task_config(config: &TaskConfig) -> Result<String> {
    let serialized = serde_json::to_string(config).map_err(|e| Error::Json {
        message: "Failed to serialize task config for hashing".to_string(),
        source: e,
    })?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}
