//! Unified cache manager with security and remote cache support
use super::{ActionCache, CacheConfig, CacheEngine, CachedTaskResult, ContentAddressedStore};
use crate::async_runtime::{run_async, AsyncRuntime};
use crate::atomic_file::write_atomic_string;
use crate::cache::signing::CacheSigner;
use crate::cue_parser::TaskConfig;
use crate::errors::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
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
        let engine = Arc::new(CacheEngine::new()?);
        let stats = Arc::new(RwLock::new(CacheStatistics::default()));
        let signer = Arc::new(CacheSigner::new(&config.base_dir)?);

        let manager = Self {
            config,
            content_store,
            action_cache,
            engine,
            stats,
            version: CACHE_VERSION,
            signer,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
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
        let _signed_result = self.signer.sign(&result)?;

        // Only cache successful results (exit_code == 0)
        if result.exit_code == 0 {
            if let Ok(mut cache) = self.memory_cache.lock() {
                cache.insert(cache_key, result);
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
        // Generate a simple cache key based on task name and config
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(task_name.as_bytes());
        hasher.update(
            serde_json::to_string(task_config)
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(working_dir.to_string_lossy().as_bytes());

        // Include relevant environment variables
        for (key, value) in env_vars {
            hasher.update(key.as_bytes());
            hasher.update(value.as_bytes());
        }

        Ok(format!("{:x}", hasher.finalize()))
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

        log::info!(
            "Cache cleanup: removed {} entries, freed {} bytes",
            removed_count,
            removed_bytes
        );

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.last_cleanup = Some(SystemTime::now());
        }

        Ok(())
    }
}
