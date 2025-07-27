//! Action cache client with remote backend support
//!
//! This module provides an action cache client that stores and retrieves
//! action results, supporting both local and remote backends.

use crate::remote_cache::proto::{Action, ActionResult, Digest};
use crate::remote_cache::{CASClient, RemoteCacheError, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Remote backend type
#[derive(Debug, Clone)]
pub enum RemoteBackend {
    /// HTTP/REST backend
    Http { url: String },
    /// gRPC backend
    Grpc { address: String },
    /// Custom backend
    Custom { name: String, config: String },
}

/// Cache client configuration
#[derive(Debug, Clone)]
pub struct CacheClientConfig {
    /// Local cache directory
    pub cache_dir: PathBuf,
    /// Remote backend configuration
    pub remote_backend: Option<RemoteBackend>,
    /// Cache TTL
    pub ttl: Duration,
    /// Maximum cache entries
    pub max_entries: usize,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for CacheClientConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from(".cuenv/action-cache"),
            remote_backend: None,
            ttl: Duration::from_secs(86400 * 7), // 7 days
            max_entries: 10000,
            enable_metrics: true,
        }
    }
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    result: ActionResult,
    action: Action,
    created_at: SystemTime,
    last_accessed: SystemTime,
    access_count: u64,
}

/// Action cache client
pub struct CacheClient {
    config: CacheClientConfig,
    local_cache: Arc<DashMap<String, CacheEntry>>,
    cas_client: Arc<CASClient>,
    remote_client: Option<Arc<dyn RemoteCacheBackend>>,
    stats: Arc<CacheStats>,
}

impl CacheClient {
    /// Create a new cache client
    pub async fn new(config: CacheClientConfig, cas_client: Arc<CASClient>) -> Result<Self> {
        // Create remote client if configured
        let remote_client = match &config.remote_backend {
            Some(RemoteBackend::Http { url }) => {
                Some(Arc::new(HttpCacheBackend::new(url.clone())?) as Arc<dyn RemoteCacheBackend>)
            }
            Some(RemoteBackend::Grpc { address }) => {
                Some(Arc::new(GrpcCacheBackend::new(address.clone())?)
                    as Arc<dyn RemoteCacheBackend>)
            }
            Some(RemoteBackend::Custom { .. }) => {
                return Err(RemoteCacheError::Configuration(
                    "Custom backends not yet implemented".to_string(),
                ))
            }
            None => None,
        };

        let client = Self {
            config,
            local_cache: Arc::new(DashMap::new()),
            cas_client,
            remote_client,
            stats: Arc::new(CacheStats::default()),
        };

        // Load local cache
        client.load_cache().await?;

        Ok(client)
    }

    /// Get action result from cache
    pub async fn get_action_result(&self, action_digest: &Digest) -> Result<Option<ActionResult>> {
        let key = &action_digest.hash;

        // Check local cache first
        if let Some(mut entry) = self.local_cache.get_mut(key) {
            entry.last_accessed = SystemTime::now();
            entry.access_count += 1;
            self.stats
                .hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Check TTL
            if let Ok(age) = SystemTime::now().duration_since(entry.created_at) {
                if age < self.config.ttl {
                    return Ok(Some(entry.result.clone()));
                }
            }

            // Entry expired, remove it
            drop(entry);
            self.local_cache.remove(key);
        }

        // Check remote cache
        if let Some(remote) = &self.remote_client {
            if let Some(result) = remote.get_action_result(action_digest).await? {
                // Verify all referenced objects exist in CAS
                if self.verify_action_result(&result).await? {
                    // Cache locally
                    let entry = CacheEntry {
                        result: result.clone(),
                        action: Action {
                            command_digest: Digest::default(),
                            input_root_digest: Digest::default(),
                            timeout: None,
                            do_not_cache: false,
                            platform: Default::default(),
                        },
                        created_at: SystemTime::now(),
                        last_accessed: SystemTime::now(),
                        access_count: 1,
                    };

                    self.local_cache.insert(key.clone(), entry);
                    self.stats
                        .remote_hits
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    return Ok(Some(result));
                }
            }
        }

        self.stats
            .misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(None)
    }

    /// Update action result in cache
    pub async fn update_action_result(
        &self,
        action_digest: &Digest,
        action: &Action,
        result: &ActionResult,
    ) -> Result<()> {
        // Don't cache if action says not to
        if action.do_not_cache {
            return Ok(());
        }

        let key = action_digest.hash.clone();

        // Store locally
        let entry = CacheEntry {
            result: result.clone(),
            action: action.clone(),
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
        };

        self.local_cache.insert(key.clone(), entry);
        self.stats
            .updates
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Update remote cache
        if let Some(remote) = &self.remote_client {
            tokio::spawn({
                let remote = remote.clone();
                let digest = action_digest.clone();
                let action = action.clone();
                let result = result.clone();
                async move {
                    let _ = remote.update_action_result(&digest, &action, &result).await;
                }
            });
        }

        // Enforce max entries
        if self.local_cache.len() > self.config.max_entries {
            self.evict_lru();
        }

        Ok(())
    }

    /// Verify that all objects referenced by an action result exist
    async fn verify_action_result(&self, result: &ActionResult) -> Result<bool> {
        let mut digests = Vec::new();

        // Add output file digests
        for output in &result.output_files {
            digests.push(output.digest.clone());
        }

        // Add output directory digests
        for output in &result.output_directories {
            digests.push(output.tree_digest.clone());
        }

        // Add stdout/stderr digests
        if let Some(digest) = &result.stdout_digest {
            digests.push(digest.clone());
        }
        if let Some(digest) = &result.stderr_digest {
            digests.push(digest.clone());
        }

        if digests.is_empty() {
            return Ok(true);
        }

        // Check all digests exist
        let exists = self.cas_client.contains_batch(&digests).await?;
        Ok(exists.iter().all(|&e| e))
    }

    /// Evict least recently used entries
    fn evict_lru(&self) {
        let target = (self.config.max_entries as f64 * 0.9) as usize;

        while self.local_cache.len() > target {
            // Find LRU entry
            let lru_key = self
                .local_cache
                .iter()
                .min_by_key(|entry| entry.value().last_accessed)
                .map(|entry| entry.key().clone());

            if let Some(key) = lru_key {
                self.local_cache.remove(&key);
                self.stats
                    .evictions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            } else {
                break;
            }
        }
    }

    /// Load cache from disk
    async fn load_cache(&self) -> Result<()> {
        let cache_file = self.config.cache_dir.join("action-cache.json");
        if !cache_file.exists() {
            return Ok(());
        }

        let data = tokio::fs::read_to_string(&cache_file).await?;
        let entries: Vec<(String, CacheEntry)> =
            serde_json::from_str(&data).map_err(|e| RemoteCacheError::Serialization(e))?;

        for (key, entry) in entries {
            // Check TTL
            if let Ok(age) = SystemTime::now().duration_since(entry.created_at) {
                if age < self.config.ttl {
                    self.local_cache.insert(key, entry);
                }
            }
        }

        Ok(())
    }

    /// Save cache to disk
    pub async fn save_cache(&self) -> Result<()> {
        let cache_file = self.config.cache_dir.join("action-cache.json");

        let entries: Vec<(String, CacheEntry)> = self
            .local_cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let data = serde_json::to_string_pretty(&entries)?;

        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        crate::atomic_file::write_atomic_string(&cache_file, &data)?;

        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStatsSnapshot {
        CacheStatsSnapshot {
            hits: self.stats.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.stats.misses.load(std::sync::atomic::Ordering::Relaxed),
            remote_hits: self
                .stats
                .remote_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            updates: self
                .stats
                .updates
                .load(std::sync::atomic::Ordering::Relaxed),
            evictions: self
                .stats
                .evictions
                .load(std::sync::atomic::Ordering::Relaxed),
            entries: self.local_cache.len(),
        }
    }

    /// Clear cache
    pub fn clear(&self) {
        self.local_cache.clear();
        self.stats.clear();
    }
}

/// Remote cache backend trait
#[async_trait::async_trait]
trait RemoteCacheBackend: Send + Sync {
    /// Get action result
    async fn get_action_result(&self, digest: &Digest) -> Result<Option<ActionResult>>;

    /// Update action result
    async fn update_action_result(
        &self,
        digest: &Digest,
        action: &Action,
        result: &ActionResult,
    ) -> Result<()>;
}

/// HTTP cache backend
struct HttpCacheBackend {
    base_url: String,
    client: reqwest::Client,
}

impl HttpCacheBackend {
    fn new(base_url: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        Ok(Self { base_url, client })
    }
}

#[async_trait::async_trait]
impl RemoteCacheBackend for HttpCacheBackend {
    async fn get_action_result(&self, digest: &Digest) -> Result<Option<ActionResult>> {
        let url = format!("{}/ac/{}", self.base_url, digest.hash);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(RemoteCacheError::Network(format!(
                "Failed to get action result: {}",
                response.status()
            )));
        }

        let result = response
            .json()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        Ok(Some(result))
    }

    async fn update_action_result(
        &self,
        digest: &Digest,
        _action: &Action,
        result: &ActionResult,
    ) -> Result<()> {
        let url = format!("{}/ac/{}", self.base_url, digest.hash);

        let response = self
            .client
            .put(&url)
            .json(result)
            .send()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RemoteCacheError::Network(format!(
                "Failed to update action result: {}",
                response.status()
            )));
        }

        Ok(())
    }
}

/// gRPC cache backend (stub implementation)
struct GrpcCacheBackend {
    _address: String,
}

impl GrpcCacheBackend {
    fn new(address: String) -> Result<Self> {
        // TODO: Implement gRPC client
        Ok(Self { _address: address })
    }
}

#[async_trait::async_trait]
impl RemoteCacheBackend for GrpcCacheBackend {
    async fn get_action_result(&self, _digest: &Digest) -> Result<Option<ActionResult>> {
        // TODO: Implement gRPC calls
        Ok(None)
    }

    async fn update_action_result(
        &self,
        _digest: &Digest,
        _action: &Action,
        _result: &ActionResult,
    ) -> Result<()> {
        // TODO: Implement gRPC calls
        Ok(())
    }
}

/// Cache statistics
#[derive(Default)]
struct CacheStats {
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
    remote_hits: std::sync::atomic::AtomicU64,
    updates: std::sync::atomic::AtomicU64,
    evictions: std::sync::atomic::AtomicU64,
}

impl CacheStats {
    fn clear(&self) {
        self.hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.misses.store(0, std::sync::atomic::Ordering::Relaxed);
        self.remote_hits
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.updates.store(0, std::sync::atomic::Ordering::Relaxed);
        self.evictions
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Cache statistics snapshot
#[derive(Debug, Clone)]
pub struct CacheStatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub remote_hits: u64,
    pub updates: u64,
    pub evictions: u64,
    pub entries: usize,
}

// Implement Serialize/Deserialize for cache persistence
impl serde::Serialize for CacheEntry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("CacheEntry", 5)?;
        state.serialize_field("result", &self.result)?;
        state.serialize_field("action", &self.action)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("last_accessed", &self.last_accessed)?;
        state.serialize_field("access_count", &self.access_count)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for CacheEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct CacheEntryData {
            result: ActionResult,
            action: Action,
            created_at: SystemTime,
            last_accessed: SystemTime,
            access_count: u64,
        }

        let data = CacheEntryData::deserialize(deserializer)?;

        Ok(CacheEntry {
            result: data.result,
            action: data.action,
            created_at: data.created_at,
            last_accessed: data.last_accessed,
            access_count: data.access_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_cache::{ActionDigest, CASClientConfig, DigestFunction};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cache_client_basic() {
        let temp_dir = TempDir::new().unwrap();
        let cas_config = CASClientConfig {
            cache_dir: temp_dir.path().join("cas"),
            ..Default::default()
        };
        let cas_client = Arc::new(CASClient::new(cas_config).await.unwrap());

        let cache_config = CacheClientConfig {
            cache_dir: temp_dir.path().join("cache"),
            ..Default::default()
        };

        let cache = CacheClient::new(cache_config, cas_client).await.unwrap();

        // Create test action and result
        let action_digest = Digest {
            hash: "test_action".to_string(),
            size_bytes: 100,
        };

        let action = Action {
            command_digest: Digest {
                hash: "test_command".to_string(),
                size_bytes: 50,
            },
            input_root_digest: Digest {
                hash: "test_input".to_string(),
                size_bytes: 200,
            },
            timeout: None,
            do_not_cache: false,
            platform: Default::default(),
        };

        let result = ActionResult {
            output_files: vec![],
            output_directories: vec![],
            exit_code: 0,
            stdout_digest: None,
            stderr_digest: None,
            execution_metadata: crate::remote_cache::proto::ExecutionMetadata {
                worker: "test_worker".to_string(),
                queued_timestamp: SystemTime::now(),
                worker_start_timestamp: SystemTime::now(),
                worker_completed_timestamp: SystemTime::now(),
                input_fetch_start_timestamp: SystemTime::now(),
                input_fetch_completed_timestamp: SystemTime::now(),
                execution_start_timestamp: SystemTime::now(),
                execution_completed_timestamp: SystemTime::now(),
                output_upload_start_timestamp: SystemTime::now(),
                output_upload_completed_timestamp: SystemTime::now(),
            },
        };

        // Cache miss
        assert!(cache
            .get_action_result(&action_digest)
            .await
            .unwrap()
            .is_none());

        // Update cache
        cache
            .update_action_result(&action_digest, &action, &result)
            .await
            .unwrap();

        // Cache hit
        let cached = cache.get_action_result(&action_digest).await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().exit_code, 0);

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.updates, 1);
    }

    #[tokio::test]
    async fn test_cache_ttl() {
        let temp_dir = TempDir::new().unwrap();
        let cas_config = CASClientConfig {
            cache_dir: temp_dir.path().join("cas"),
            ..Default::default()
        };
        let cas_client = Arc::new(CASClient::new(cas_config).await.unwrap());

        let cache_config = CacheClientConfig {
            cache_dir: temp_dir.path().join("cache"),
            ttl: Duration::from_millis(100), // Very short TTL
            ..Default::default()
        };

        let cache = CacheClient::new(cache_config, cas_client).await.unwrap();

        let action_digest = Digest {
            hash: "test_ttl".to_string(),
            size_bytes: 100,
        };

        let action = Action {
            command_digest: Digest::default(),
            input_root_digest: Digest::default(),
            timeout: None,
            do_not_cache: false,
            platform: Default::default(),
        };

        let result = ActionResult {
            output_files: vec![],
            output_directories: vec![],
            exit_code: 0,
            stdout_digest: None,
            stderr_digest: None,
            execution_metadata: Default::default(),
        };

        // Update cache
        cache
            .update_action_result(&action_digest, &action, &result)
            .await
            .unwrap();

        // Should be cached
        assert!(cache
            .get_action_result(&action_digest)
            .await
            .unwrap()
            .is_some());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache
            .get_action_result(&action_digest)
            .await
            .unwrap()
            .is_none());
    }
}
