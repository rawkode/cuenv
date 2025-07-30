//! Integration layer between local and remote caches
//!
//! This module provides a unified cache interface that transparently handles
//! both local and remote cache operations with proper fallback behavior.

use crate::cache::{ActionCache, ContentAddressedStore};
use crate::errors::{Error, Result};
use crate::remote_cache::{RemoteCacheClient, RemoteCacheClientConfig};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for cache integration
#[derive(Debug, Clone)]
pub struct CacheIntegrationConfig {
    /// Remote cache configuration
    pub remote_config: Option<RemoteCacheClientConfig>,
    /// Whether to upload local cache misses to remote
    pub upload_on_miss: bool,
    /// Whether to populate local cache from remote hits
    pub populate_local: bool,
    /// Maximum concurrent remote operations
    pub max_concurrent_ops: usize,
    /// Remote operation timeout
    pub remote_timeout: Duration,
    /// Whether to fail if remote is unavailable
    pub fail_on_remote_error: bool,
}

impl Default for CacheIntegrationConfig {
    fn default() -> Self {
        Self {
            remote_config: None,
            upload_on_miss: true,
            populate_local: true,
            max_concurrent_ops: 10,
            remote_timeout: Duration::from_secs(30),
            fail_on_remote_error: false,
        }
    }
}

/// Statistics for integrated cache operations
#[derive(Debug, Default)]
pub struct IntegratedCacheStats {
    pub local_hits: u64,
    pub local_misses: u64,
    pub remote_hits: u64,
    pub remote_misses: u64,
    pub remote_uploads: u64,
    pub remote_errors: u64,
    pub total_bytes_downloaded: u64,
    pub total_bytes_uploaded: u64,
}

/// Integrated cache that combines local and remote caching
pub struct IntegratedCache {
    /// Local content-addressed store
    local_cas: Arc<ContentAddressedStore>,
    /// Local action cache
    local_action_cache: Arc<ActionCache>,
    /// Remote cache client (if configured)
    remote_client: Arc<RwLock<Option<RemoteCacheClient>>>,
    /// Configuration
    config: CacheIntegrationConfig,
    /// Statistics
    stats: Arc<RwLock<IntegratedCacheStats>>,
}

impl IntegratedCache {
    /// Create a new integrated cache
    pub async fn new(
        local_cas: Arc<ContentAddressedStore>,
        local_action_cache: Arc<ActionCache>,
        config: CacheIntegrationConfig,
    ) -> Result<Self> {
        // Initialize remote client if configured
        let remote_client = if let Some(ref remote_config) = config.remote_config {
            match RemoteCacheClient::new(remote_config.clone()).await {
                Ok(client) => {
                    info!(
                        "Connected to remote cache: {}",
                        remote_config.server_address
                    );
                    Some(client)
                }
                Err(e) => {
                    if config.fail_on_remote_error {
                        return Err(Error::configuration(format!(
                            "Failed to connect to remote cache: {}",
                            e
                        )));
                    } else {
                        warn!("Failed to connect to remote cache: {}. Continuing with local cache only.", e);
                        None
                    }
                }
            }
        } else {
            None
        };

        Ok(Self {
            local_cas,
            local_action_cache,
            remote_client: Arc::new(RwLock::new(remote_client)),
            config,
            stats: Arc::new(RwLock::new(IntegratedCacheStats::default())),
        })
    }

    /// Store content in both local and remote caches
    pub async fn store_content(&self, content: &[u8]) -> Result<String> {
        let start = Instant::now();

        // Always store locally first
        let hash = self.local_cas.store(Cursor::new(content))?;

        // Upload to remote if configured
        if self.config.upload_on_miss {
            if let Some(ref mut client) = *self.remote_client.write() {
                let digest = super::grpc_proto::proto::Digest {
                    hash: hash.clone(),
                    size_bytes: content.len() as i64,
                };

                match tokio::time::timeout(
                    self.config.remote_timeout,
                    client.upload_blobs("", vec![(digest, content.to_vec())]),
                )
                .await
                {
                    Ok(Ok(results)) => {
                        let success_count = results.iter().filter(|(_, success)| *success).count();
                        if success_count > 0 {
                            self.stats.write().remote_uploads += success_count as u64;
                            self.stats.write().total_bytes_uploaded += content.len() as u64;
                            debug!(
                                "Uploaded {} bytes to remote cache in {:?}",
                                content.len(),
                                start.elapsed()
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        self.stats.write().remote_errors += 1;
                        warn!("Failed to upload to remote cache: {}", e);
                    }
                    Err(_) => {
                        self.stats.write().remote_errors += 1;
                        warn!("Remote cache upload timed out");
                    }
                }
            }
        }

        Ok(hash)
    }

    /// Retrieve content from local or remote cache
    pub async fn retrieve_content(&self, hash: &str) -> Result<Vec<u8>> {
        let start = Instant::now();

        // Try local cache first
        match self.local_cas.retrieve(hash) {
            Ok(content) => {
                self.stats.write().local_hits += 1;
                debug!("Local cache hit for {} in {:?}", hash, start.elapsed());
                return Ok(content);
            }
            Err(_) => {
                self.stats.write().local_misses += 1;
            }
        }

        // Try remote cache if local miss
        if let Some(ref mut client) = *self.remote_client.write() {
            let digest = super::grpc_proto::proto::Digest {
                hash: hash.to_string(),
                size_bytes: 0, // We don't know the size
            };

            match tokio::time::timeout(
                self.config.remote_timeout,
                client.download_blobs("", vec![digest]),
            )
            .await
            {
                Ok(Ok(results)) => {
                    for (digest, data) in results {
                        if let Some(content) = data {
                            self.stats.write().remote_hits += 1;
                            self.stats.write().total_bytes_downloaded += content.len() as u64;

                            // Populate local cache if configured
                            if self.config.populate_local {
                                match self.local_cas.store(Cursor::new(&content)) {
                                    Ok(_) => {
                                        debug!(
                                            "Populated local cache with {} bytes from remote",
                                            content.len()
                                        );
                                    }
                                    Err(e) => {
                                        warn!("Failed to populate local cache: {}", e);
                                    }
                                }
                            }

                            debug!(
                                "Remote cache hit for {} ({} bytes) in {:?}",
                                digest.hash,
                                content.len(),
                                start.elapsed()
                            );
                            return Ok(content);
                        }
                    }

                    self.stats.write().remote_misses += 1;
                }
                Ok(Err(e)) => {
                    self.stats.write().remote_errors += 1;
                    warn!("Failed to retrieve from remote cache: {}", e);
                }
                Err(_) => {
                    self.stats.write().remote_errors += 1;
                    warn!("Remote cache retrieval timed out");
                }
            }
        }

        Err(Error::configuration(format!(
            "Content not found in any cache: {}",
            hash
        )))
    }

    /// Check if content exists in any cache
    pub async fn contains(&self, hash: &str) -> bool {
        // Check local first
        if self.local_cas.contains(hash) {
            return true;
        }

        // Check remote
        if let Some(ref mut client) = *self.remote_client.write() {
            let digest = super::grpc_proto::proto::Digest {
                hash: hash.to_string(),
                size_bytes: 0,
            };

            match tokio::time::timeout(
                Duration::from_secs(5), // Shorter timeout for existence checks
                client.find_missing_blobs("", vec![digest.clone()]),
            )
            .await
            {
                Ok(Ok(missing)) => {
                    return !missing.iter().any(|d| d.hash == hash);
                }
                _ => {}
            }
        }

        false
    }

    /// Get statistics
    pub fn stats(&self) -> IntegratedCacheStats {
        self.stats.read().clone()
    }

    /// Clear statistics
    pub fn clear_stats(&self) {
        *self.stats.write() = IntegratedCacheStats::default();
    }

    /// Check if remote cache is available
    pub fn is_remote_available(&self) -> bool {
        self.remote_client.read().is_some()
    }

    /// Reconnect to remote cache
    pub async fn reconnect_remote(&self) -> Result<()> {
        if let Some(ref remote_config) = self.config.remote_config {
            match RemoteCacheClient::new(remote_config.clone()).await {
                Ok(client) => {
                    *self.remote_client.write() = Some(client);
                    info!("Reconnected to remote cache");
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to reconnect to remote cache: {}", e);
                    Err(Error::configuration(format!(
                        "Failed to reconnect to remote cache: {}",
                        e
                    )))
                }
            }
        } else {
            Err(Error::configuration(
                "No remote cache configured".to_string(),
            ))
        }
    }
}

/// Builder for IntegratedCache
pub struct IntegratedCacheBuilder {
    config: CacheIntegrationConfig,
    local_cas: Option<Arc<ContentAddressedStore>>,
    local_action_cache: Option<Arc<ActionCache>>,
}

impl IntegratedCacheBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: CacheIntegrationConfig::default(),
            local_cas: None,
            local_action_cache: None,
        }
    }

    /// Set local CAS
    pub fn local_cas(mut self, cas: Arc<ContentAddressedStore>) -> Self {
        self.local_cas = Some(cas);
        self
    }

    /// Set local action cache
    pub fn local_action_cache(mut self, cache: Arc<ActionCache>) -> Self {
        self.local_action_cache = Some(cache);
        self
    }

    /// Set remote cache configuration
    pub fn remote_config(mut self, config: RemoteCacheClientConfig) -> Self {
        self.config.remote_config = Some(config);
        self
    }

    /// Set whether to upload on miss
    pub fn upload_on_miss(mut self, enabled: bool) -> Self {
        self.config.upload_on_miss = enabled;
        self
    }

    /// Set whether to populate local cache
    pub fn populate_local(mut self, enabled: bool) -> Self {
        self.config.populate_local = enabled;
        self
    }

    /// Set maximum concurrent operations
    pub fn max_concurrent_ops(mut self, max: usize) -> Self {
        self.config.max_concurrent_ops = max;
        self
    }

    /// Set remote timeout
    pub fn remote_timeout(mut self, timeout: Duration) -> Self {
        self.config.remote_timeout = timeout;
        self
    }

    /// Set whether to fail on remote error
    pub fn fail_on_remote_error(mut self, fail: bool) -> Self {
        self.config.fail_on_remote_error = fail;
        self
    }

    /// Build the integrated cache
    pub async fn build(self) -> Result<IntegratedCache> {
        let local_cas = self
            .local_cas
            .ok_or_else(|| Error::configuration("Local CAS not provided".to_string()))?;

        let local_action_cache = self
            .local_action_cache
            .ok_or_else(|| Error::configuration("Local action cache not provided".to_string()))?;

        IntegratedCache::new(local_cas, local_action_cache, self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_integrated_cache_local_only() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Arc::new(ContentAddressedStore::new(temp_dir.path().join("cas"), 1024).unwrap());
        let action_cache =
            Arc::new(ActionCache::new(cas.clone(), 1024 * 1024, temp_dir.path()).unwrap());

        let cache = IntegratedCache::new(cas, action_cache, CacheIntegrationConfig::default())
            .await
            .unwrap();

        // Test store and retrieve
        let content = b"test content";
        let hash = cache.store_content(content).await.unwrap();
        let retrieved = cache.retrieve_content(&hash).await.unwrap();

        assert_eq!(retrieved, content);

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.local_hits, 1);
        assert_eq!(stats.local_misses, 0);
    }

    #[tokio::test]
    async fn test_integrated_cache_builder() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Arc::new(ContentAddressedStore::new(temp_dir.path().join("cas"), 1024).unwrap());
        let action_cache =
            Arc::new(ActionCache::new(cas.clone(), 1024 * 1024, temp_dir.path()).unwrap());

        let cache = IntegratedCacheBuilder::new()
            .local_cas(cas)
            .local_action_cache(action_cache)
            .upload_on_miss(false)
            .populate_local(false)
            .max_concurrent_ops(5)
            .remote_timeout(Duration::from_secs(10))
            .build()
            .await
            .unwrap();

        assert!(!cache.is_remote_available());
    }
}
