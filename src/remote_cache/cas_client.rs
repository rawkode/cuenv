//! Content-Addressed Storage client with remote backend support
//!
//! This module provides a CAS client that can work with both local
//! and remote storage backends, implementing the Remote Execution API
//! CAS operations.

use crate::remote_cache::proto::{Digest, Directory, FileNode};
use crate::remote_cache::{RemoteCacheError, Result};
use dashmap::DashMap;
use futures::stream::{self, StreamExt};
use parking_lot::RwLock;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// CAS client configuration
#[derive(Debug, Clone)]
pub struct CASClientConfig {
    /// Local cache directory
    pub cache_dir: PathBuf,
    /// Remote backend URL (if any)
    pub remote_url: Option<String>,
    /// Maximum local cache size in bytes
    pub max_cache_size: u64,
    /// Chunk size for uploads/downloads
    pub chunk_size: usize,
    /// Request timeout
    pub timeout: Duration,
    /// Number of concurrent operations
    pub concurrency: usize,
    /// Enable compression
    pub compression: bool,
}

impl Default for CASClientConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from(".cuenv/cas"),
            remote_url: None,
            max_cache_size: 10 * 1024 * 1024 * 1024, // 10GB
            chunk_size: 4 * 1024 * 1024,             // 4MB
            timeout: Duration::from_secs(300),
            concurrency: 4,
            compression: true,
        }
    }
}

/// Blob metadata
#[derive(Debug, Clone)]
struct BlobMetadata {
    digest: Digest,
    last_accessed: Instant,
    ref_count: u64,
    compressed_size: Option<u64>,
}

/// CAS client implementation
pub struct CASClient {
    config: CASClientConfig,
    local_store: Arc<LocalCASStore>,
    remote_client: Option<Arc<RemoteCASClient>>,
    stats: Arc<CASStats>,
}

impl CASClient {
    /// Create a new CAS client
    pub async fn new(config: CASClientConfig) -> Result<Self> {
        // Create local store
        let local_store = Arc::new(LocalCASStore::new(&config).await?);

        // Create remote client if configured
        let remote_client = if let Some(url) = &config.remote_url {
            Some(Arc::new(RemoteCASClient::new(url.clone(), &config)?))
        } else {
            None
        };

        Ok(Self {
            config,
            local_store,
            remote_client,
            stats: Arc::new(CASStats::default()),
        })
    }

    /// Check if a blob exists
    pub async fn contains(&self, digest: &Digest) -> Result<bool> {
        // Check local first
        if self.local_store.contains(digest).await? {
            self.stats.local_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(true);
        }

        // Check remote if available
        if let Some(remote) = &self.remote_client {
            if remote.contains(digest).await? {
                self.stats.remote_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(true);
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        Ok(false)
    }

    /// Get a blob by digest
    pub async fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        // Try local first
        if let Ok(data) = self.local_store.get(digest).await {
            self.stats.local_hits.fetch_add(1, Ordering::Relaxed);
            self.stats
                .bytes_read
                .fetch_add(data.len() as u64, Ordering::Relaxed);
            return Ok(data);
        }

        // Try remote
        if let Some(remote) = &self.remote_client {
            let data = remote.get(digest).await?;

            // Cache locally
            self.local_store.put(digest, &data).await?;

            self.stats.remote_hits.fetch_add(1, Ordering::Relaxed);
            self.stats
                .bytes_downloaded
                .fetch_add(data.len() as u64, Ordering::Relaxed);
            return Ok(data);
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        Err(RemoteCacheError::ObjectNotFound(digest.hash.clone()))
    }

    /// Put a blob into CAS
    pub async fn put(&self, data: &[u8]) -> Result<Digest> {
        use sha2::{Digest as Sha2Digest, Sha256};

        // Compute digest
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = format!("{:x}", hasher.finalize());
        let digest = Digest {
            hash,
            size_bytes: data.len() as i64,
        };

        // Store locally
        self.local_store.put(&digest, data).await?;
        self.stats
            .bytes_written
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        // Upload to remote if configured
        if let Some(remote) = &self.remote_client {
            tokio::spawn({
                let remote = remote.clone();
                let digest = digest.clone();
                let data = data.to_vec();
                let stats = self.stats.clone();
                async move {
                    if let Ok(_) = remote.put(&digest, &data).await {
                        stats
                            .bytes_uploaded
                            .fetch_add(data.len() as u64, Ordering::Relaxed);
                    }
                }
            });
        }

        Ok(digest)
    }

    /// Put a stream into CAS
    pub async fn put_stream<R: AsyncRead + Unpin>(&self, mut reader: R) -> Result<Digest> {
        use sha2::{Digest as Sha2Digest, Sha256};

        let mut hasher = Sha256::new();
        let mut data = Vec::new();
        let mut buffer = vec![0u8; 8192];

        loop {
            let n = reader.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            data.extend_from_slice(&buffer[..n]);
        }

        let hash = format!("{:x}", hasher.finalize());
        let digest = Digest {
            hash,
            size_bytes: data.len() as i64,
        };

        self.local_store.put(&digest, &data).await?;
        self.stats
            .bytes_written
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(digest)
    }

    /// Get a blob as a stream
    pub async fn get_stream(&self, digest: &Digest) -> Result<impl AsyncRead> {
        let data = self.get(digest).await?;
        Ok(Cursor::new(data))
    }

    /// Batch check for blob existence
    pub async fn contains_batch(&self, digests: &[Digest]) -> Result<Vec<bool>> {
        let mut results = vec![false; digests.len()];

        // Check local store in parallel
        let local_checks = stream::iter(digests.iter().enumerate())
            .map(|(i, digest)| async move {
                (i, self.local_store.contains(digest).await.unwrap_or(false))
            })
            .buffer_unordered(self.config.concurrency);

        let local_results: Vec<_> = local_checks.collect().await;

        for (i, exists) in local_results {
            results[i] = exists;
            if exists {
                self.stats.local_hits.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Check remote for missing blobs
        if let Some(remote) = &self.remote_client {
            let missing: Vec<_> = digests
                .iter()
                .enumerate()
                .filter(|(i, _)| !results[*i])
                .map(|(i, d)| (i, d.clone()))
                .collect();

            if !missing.is_empty() {
                let remote_checks = stream::iter(missing.iter())
                    .map(|(i, digest)| async move {
                        (*i, remote.contains(digest).await.unwrap_or(false))
                    })
                    .buffer_unordered(self.config.concurrency);

                let remote_results: Vec<_> = remote_checks.collect().await;

                for (i, exists) in remote_results {
                    results[i] = exists;
                    if exists {
                        self.stats.remote_hits.fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.stats.misses.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Put a directory tree into CAS
    pub async fn put_directory(&self, path: &Path) -> Result<Digest> {
        let tree = build_directory_tree(path).await?;
        self.put_directory_proto(&tree).await
    }

    /// Put a directory proto into CAS
    pub async fn put_directory_proto(&self, dir: &Directory) -> Result<Digest> {
        let data = serde_json::to_vec(dir)?;
        self.put(&data).await
    }

    /// Get a directory proto from CAS
    pub async fn get_directory(&self, digest: &Digest) -> Result<Directory> {
        let data = self.get(digest).await?;
        Ok(serde_json::from_slice(&data)?)
    }

    /// Get statistics
    pub fn stats(&self) -> CASStatsSnapshot {
        self.stats.snapshot()
    }

    /// Clear local cache
    pub async fn clear_cache(&self) -> Result<()> {
        self.local_store.clear().await
    }

    /// Run garbage collection
    pub async fn gc(&self) -> Result<(usize, u64)> {
        self.local_store.gc().await
    }
}

/// Local CAS store
struct LocalCASStore {
    base_dir: PathBuf,
    index: Arc<DashMap<String, BlobMetadata>>,
    total_size: AtomicU64,
    max_size: u64,
    compression: bool,
}

impl LocalCASStore {
    async fn new(config: &CASClientConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&config.cache_dir).await?;

        let store = Self {
            base_dir: config.cache_dir.clone(),
            index: Arc::new(DashMap::new()),
            total_size: AtomicU64::new(0),
            max_size: config.max_cache_size,
            compression: config.compression,
        };

        // Load existing index
        store.load_index().await?;

        Ok(store)
    }

    async fn contains(&self, digest: &Digest) -> Result<bool> {
        if self.index.contains_key(&digest.hash) {
            // Update access time
            if let Some(mut entry) = self.index.get_mut(&digest.hash) {
                entry.last_accessed = Instant::now();
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        let path = self.blob_path(&digest.hash);

        if !path.exists() {
            return Err(RemoteCacheError::ObjectNotFound(digest.hash.clone()));
        }

        // Update access time
        if let Some(mut entry) = self.index.get_mut(&digest.hash) {
            entry.last_accessed = Instant::now();
        }

        let data = tokio::fs::read(&path).await?;

        // Decompress if needed
        if self.compression {
            use flate2::read::GzDecoder;
            use std::io::Read;

            let mut decoder = GzDecoder::new(&data[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| RemoteCacheError::CAS(format!("Decompression failed: {}", e)))?;
            Ok(decompressed)
        } else {
            Ok(data)
        }
    }

    async fn put(&self, digest: &Digest, data: &[u8]) -> Result<()> {
        // Check if already exists
        if self.contains(digest).await? {
            return Ok(());
        }

        let path = self.blob_path(&digest.hash);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Compress if enabled
        let (data_to_store, compressed_size) = if self.compression {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            use std::io::Write;

            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(data)
                .map_err(|e| RemoteCacheError::CAS(format!("Compression failed: {}", e)))?;
            let compressed = encoder
                .finish()
                .map_err(|e| RemoteCacheError::CAS(format!("Compression failed: {}", e)))?;
            let size = compressed.len() as u64;
            (compressed, Some(size))
        } else {
            (data.to_vec(), None)
        };

        // Write atomically
        crate::atomic_file::write_atomic(&path, &data_to_store)?;

        // Update index
        let metadata = BlobMetadata {
            digest: digest.clone(),
            last_accessed: Instant::now(),
            ref_count: 1,
            compressed_size,
        };

        self.index.insert(digest.hash.clone(), metadata);
        self.total_size.fetch_add(
            compressed_size.unwrap_or(data.len() as u64),
            Ordering::Relaxed,
        );

        // Run eviction if needed
        if self.total_size.load(Ordering::Relaxed) > self.max_size {
            tokio::spawn({
                let store = self.clone();
                async move {
                    let _ = store.evict_lru().await;
                }
            });
        }

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.index.clear();
        self.total_size.store(0, Ordering::Relaxed);
        tokio::fs::remove_dir_all(&self.base_dir).await?;
        tokio::fs::create_dir_all(&self.base_dir).await?;
        Ok(())
    }

    async fn gc(&self) -> Result<(usize, u64)> {
        let mut removed = 0;
        let mut freed = 0u64;

        let zero_ref: Vec<_> = self
            .index
            .iter()
            .filter(|e| e.ref_count == 0)
            .map(|e| e.key().clone())
            .collect();

        for hash in zero_ref {
            if let Some((_, metadata)) = self.index.remove(&hash) {
                let path = self.blob_path(&hash);
                if path.exists() {
                    tokio::fs::remove_file(&path).await?;
                    let size = metadata
                        .compressed_size
                        .unwrap_or(metadata.digest.size_bytes as u64);
                    self.total_size.fetch_sub(size, Ordering::Relaxed);
                    freed += size;
                    removed += 1;
                }
            }
        }

        Ok((removed, freed))
    }

    async fn evict_lru(&self) -> Result<()> {
        let target_size = (self.max_size as f64 * 0.8) as u64;

        while self.total_size.load(Ordering::Relaxed) > target_size {
            // Find oldest entry
            let oldest = self
                .index
                .iter()
                .min_by_key(|e| e.last_accessed)
                .map(|e| e.key().clone());

            if let Some(hash) = oldest {
                if let Some((_, metadata)) = self.index.remove(&hash) {
                    let path = self.blob_path(&hash);
                    if path.exists() {
                        tokio::fs::remove_file(&path).await?;
                        let size = metadata
                            .compressed_size
                            .unwrap_or(metadata.digest.size_bytes as u64);
                        self.total_size.fetch_sub(size, Ordering::Relaxed);
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        // Use sharding to avoid too many files in one directory
        let (shard, name) = hash.split_at(2.min(hash.len()));
        self.base_dir.join("blobs").join(shard).join(name)
    }

    async fn load_index(&self) -> Result<()> {
        let index_path = self.base_dir.join("index.json");
        if !index_path.exists() {
            return Ok(());
        }

        let data = tokio::fs::read_to_string(&index_path).await?;
        let entries: Vec<BlobMetadata> = serde_json::from_str(&data)?;

        let mut total = 0u64;
        for entry in entries {
            let size = entry
                .compressed_size
                .unwrap_or(entry.digest.size_bytes as u64);
            total += size;
            self.index.insert(entry.digest.hash.clone(), entry);
        }

        self.total_size.store(total, Ordering::Relaxed);
        Ok(())
    }
}

// Clone implementation for LocalCASStore
impl Clone for LocalCASStore {
    fn clone(&self) -> Self {
        Self {
            base_dir: self.base_dir.clone(),
            index: self.index.clone(),
            total_size: AtomicU64::new(self.total_size.load(Ordering::Relaxed)),
            max_size: self.max_size,
            compression: self.compression,
        }
    }
}

/// Remote CAS client
struct RemoteCASClient {
    base_url: String,
    client: reqwest::Client,
    timeout: Duration,
}

impl RemoteCASClient {
    fn new(base_url: String, config: &CASClientConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        Ok(Self {
            base_url,
            client,
            timeout: config.timeout,
        })
    }

    async fn contains(&self, digest: &Digest) -> Result<bool> {
        let url = format!(
            "{}/blobs/{}/{}",
            self.base_url, digest.hash, digest.size_bytes
        );

        let response = self
            .client
            .head(&url)
            .send()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        Ok(response.status().is_success())
    }

    async fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        let url = format!(
            "{}/blobs/{}/{}",
            self.base_url, digest.hash, digest.size_bytes
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RemoteCacheError::ObjectNotFound(digest.hash.clone()));
        }

        let data = response
            .bytes()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        Ok(data.to_vec())
    }

    async fn put(&self, digest: &Digest, data: &[u8]) -> Result<()> {
        let url = format!(
            "{}/uploads/{}/blobs/{}/{}",
            self.base_url,
            uuid::Uuid::new_v4(),
            digest.hash,
            digest.size_bytes
        );

        let response = self
            .client
            .put(&url)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| RemoteCacheError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RemoteCacheError::Network(format!(
                "Upload failed: {}",
                response.status()
            )));
        }

        Ok(())
    }
}

/// CAS statistics
#[derive(Default)]
struct CASStats {
    local_hits: AtomicU64,
    remote_hits: AtomicU64,
    misses: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
    bytes_uploaded: AtomicU64,
    bytes_downloaded: AtomicU64,
}

impl CASStats {
    fn snapshot(&self) -> CASStatsSnapshot {
        CASStatsSnapshot {
            local_hits: self.local_hits.load(Ordering::Relaxed),
            remote_hits: self.remote_hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            bytes_uploaded: self.bytes_uploaded.load(Ordering::Relaxed),
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
        }
    }
}

/// CAS statistics snapshot
#[derive(Debug, Clone)]
pub struct CASStatsSnapshot {
    pub local_hits: u64,
    pub remote_hits: u64,
    pub misses: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
}

/// Build directory tree from filesystem
async fn build_directory_tree(path: &Path) -> Result<Directory> {
    use crate::remote_cache::proto::{DirectoryNode, SymlinkNode};
    use sha2::{Digest as Sha2Digest, Sha256};

    let mut dir = Directory {
        files: Vec::new(),
        directories: Vec::new(),
        symlinks: Vec::new(),
    };

    let mut entries = tokio::fs::read_dir(path).await?;
    let mut items = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        items.push(entry);
    }

    // Sort for deterministic ordering
    items.sort_by_key(|e| e.file_name());

    for entry in items {
        let metadata = entry.metadata().await?;
        let name = entry.file_name().to_string_lossy().to_string();

        if metadata.is_file() {
            let content = tokio::fs::read(entry.path()).await?;
            let mut hasher = Sha256::new();
            hasher.update(&content);
            let hash = format!("{:x}", hasher.finalize());

            let digest = Digest {
                hash,
                size_bytes: content.len() as i64,
            };

            let is_executable = is_executable(&metadata);

            dir.files.push(FileNode {
                name,
                digest,
                is_executable,
            });
        } else if metadata.is_dir() {
            // Recursively build subdirectory
            let sub_tree = Box::pin(build_directory_tree(&entry.path())).await?;
            let tree_data = serde_json::to_vec(&sub_tree)?;

            let mut hasher = Sha256::new();
            hasher.update(&tree_data);
            let hash = format!("{:x}", hasher.finalize());

            let digest = Digest {
                hash,
                size_bytes: tree_data.len() as i64,
            };

            dir.directories.push(DirectoryNode { name, digest });
        } else if metadata.is_symlink() {
            let target = tokio::fs::read_link(entry.path())
                .await?
                .to_string_lossy()
                .to_string();
            dir.symlinks.push(SymlinkNode { name, target });
        }
    }

    Ok(dir)
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cas_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASClientConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let cas = CASClient::new(config).await.unwrap();

        // Put and get
        let data = b"Hello, CAS!";
        let digest = cas.put(data).await.unwrap();

        assert!(cas.contains(&digest).await.unwrap());

        let retrieved = cas.get(&digest).await.unwrap();
        assert_eq!(retrieved, data);

        // Stats
        let stats = cas.stats();
        assert_eq!(stats.local_hits, 1);
        assert_eq!(stats.bytes_written, data.len() as u64);
        assert_eq!(stats.bytes_read, data.len() as u64);
    }

    #[tokio::test]
    async fn test_cas_compression() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASClientConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            compression: true,
            ..Default::default()
        };

        let cas = CASClient::new(config).await.unwrap();

        // Large compressible data
        let data = vec![b'A'; 10000];
        let digest = cas.put(&data).await.unwrap();

        // Should compress well
        let retrieved = cas.get(&digest).await.unwrap();
        assert_eq!(retrieved, data);

        // Check that compressed size is smaller
        let blob_path = cas.local_store.blob_path(&digest.hash);
        let compressed_size = tokio::fs::metadata(&blob_path).await.unwrap().len();
        assert!(compressed_size < data.len() as u64);
    }

    #[tokio::test]
    async fn test_cas_batch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASClientConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let cas = CASClient::new(config).await.unwrap();

        // Put multiple blobs
        let digests: Vec<_> = futures::future::join_all((0..10).map(|i| {
            let cas = &cas;
            async move {
                let data = format!("Data {}", i);
                cas.put(data.as_bytes()).await.unwrap()
            }
        }))
        .await;

        // Batch check
        let exists = cas.contains_batch(&digests).await.unwrap();
        assert!(exists.iter().all(|&e| e));

        // Check non-existent
        let fake_digest = Digest {
            hash: "fake".to_string(),
            size_bytes: 4,
        };
        let mut check_digests = digests.clone();
        check_digests.push(fake_digest);

        let exists = cas.contains_batch(&check_digests).await.unwrap();
        assert_eq!(exists.len(), 11);
        assert!(exists[..10].iter().all(|&e| e));
        assert!(!exists[10]);
    }
}
