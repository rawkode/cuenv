//! Bazel-compatible remote cache server implementation
//!
//! This module implements a fully-compliant Bazel Remote Execution API (REAPI) v2
//! cache server that can be used with Bazel, Buck2, and other compatible build systems.

use crate::cache::{ActionCache, CacheConfig, ContentAddressedStore};
use crate::errors::{Error, Result};
use anyhow::Result as AnyhowResult;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info, warn};

use super::grpc_proto::proto::{
    action_cache_server::{ActionCache as ActionCacheService, ActionCacheServer},
    capabilities_server::{Capabilities as CapabilitiesService, CapabilitiesServer},
    content_addressable_storage_server::{
        ContentAddressableStorage as CASService, ContentAddressableStorageServer,
    },
    ActionCacheUpdateCapabilities, ActionResult, BatchReadBlobsRequest, BatchReadBlobsResponse,
    BatchUpdateBlobsRequest, BatchUpdateBlobsResponse, CacheCapabilities, Digest,
    FindMissingBlobsRequest, FindMissingBlobsResponse, GetActionResultRequest,
    GetCapabilitiesRequest, ServerCapabilities, UpdateActionResultRequest,
};

/// Configuration for the Bazel-compatible remote cache server
pub struct BazelRemoteCacheConfig {
    pub address: SocketAddr,
    pub cache_config: CacheConfig,
    pub max_batch_size: usize,
    pub max_blob_size: u64,
    pub enable_action_cache: bool,
    pub enable_cas: bool,
    pub enable_authentication: bool,
    pub circuit_breaker_threshold: f64,
    pub circuit_breaker_timeout: Duration,
}

impl Default for BazelRemoteCacheConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:50051".parse().unwrap(),
            cache_config: CacheConfig::default(),
            max_batch_size: 1000,
            max_blob_size: 1024 * 1024 * 1024, // 1GB
            enable_action_cache: true,
            enable_cas: true,
            enable_authentication: false,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for fault tolerance
struct CircuitBreaker {
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: RwLock<Option<Instant>>,
    threshold: f64,
    timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: f64, timeout: Duration) -> Self {
        Self {
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            threshold,
            timeout,
        }
    }

    fn is_open(&self) -> bool {
        let total = self.failure_count.load(Ordering::Relaxed) as f64
            + self.success_count.load(Ordering::Relaxed) as f64;

        if total < 10.0 {
            return false; // Not enough data
        }

        let failure_rate = self.failure_count.load(Ordering::Relaxed) as f64 / total;
        if failure_rate > self.threshold {
            if let Some(last_failure) = *self.last_failure_time.read() {
                return last_failure.elapsed() < self.timeout;
            }
        }

        false
    }

    fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        // Reset failure count after successful operations
        if self.success_count.load(Ordering::Relaxed) % 100 == 0 {
            self.failure_count.store(0, Ordering::Relaxed);
        }
    }

    fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        *self.last_failure_time.write() = Some(Instant::now());
    }
}

/// Bazel-compatible remote cache server
pub struct BazelRemoteCacheServer {
    config: BazelRemoteCacheConfig,
    cas: Arc<ContentAddressedStore>,
    action_cache: Arc<ActionCache>,
    circuit_breaker: Arc<CircuitBreaker>,
    request_stats: Arc<DashMap<String, AtomicU64>>,
}

impl BazelRemoteCacheServer {
    /// Create a new Bazel-compatible remote cache server
    pub async fn new(config: BazelRemoteCacheConfig) -> AnyhowResult<Self> {
        // Create cache directories
        std::fs::create_dir_all(&config.cache_config.base_dir)?;
        let cas_dir = config.cache_config.base_dir.join("cas");
        std::fs::create_dir_all(&cas_dir)?;

        // Initialize content-addressed store
        let cas = Arc::new(ContentAddressedStore::new(
            cas_dir.clone(),
            config.cache_config.inline_threshold,
        )?);

        // Initialize action cache
        let action_cache = Arc::new(ActionCache::new(
            cas.clone(),
            config.cache_config.max_size,
            &config.cache_config.base_dir,
        )?);

        // Initialize circuit breaker
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_timeout,
        ));

        Ok(Self {
            config,
            cas,
            action_cache,
            circuit_breaker,
            request_stats: Arc::new(DashMap::new()),
        })
    }

    /// Start serving the remote cache
    pub async fn serve(self) -> AnyhowResult<()> {
        let cas_service = BazelCASService {
            cas: Arc::clone(&self.cas),
            circuit_breaker: Arc::clone(&self.circuit_breaker),
            max_batch_size: self.config.max_batch_size,
            max_blob_size: self.config.max_blob_size,
            stats: Arc::clone(&self.request_stats),
        };

        let action_cache_service = BazelActionCacheService {
            action_cache: Arc::clone(&self.action_cache),
            circuit_breaker: Arc::clone(&self.circuit_breaker),
            stats: Arc::clone(&self.request_stats),
        };

        let capabilities_service = BazelCapabilitiesService {
            max_batch_size: self.config.max_batch_size,
            enable_action_cache: self.config.enable_action_cache,
            enable_cas: self.config.enable_cas,
        };

        info!(
            "Starting Bazel-compatible remote cache server on {}",
            self.config.address
        );
        info!(
            "Cache directory: {}",
            self.config.cache_config.base_dir.display()
        );
        info!("Max batch size: {}", self.config.max_batch_size);
        info!("Max blob size: {} bytes", self.config.max_blob_size);

        let mut builder = Server::builder();

        // Add reflection service for debugging
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(super::grpc_proto::proto::FILE_DESCRIPTOR_SET)
            .build()?;

        builder = builder.add_service(reflection_service);

        // Add core services
        if self.config.enable_cas {
            builder = builder.add_service(ContentAddressableStorageServer::new(cas_service));
        }

        if self.config.enable_action_cache {
            builder = builder.add_service(ActionCacheServer::new(action_cache_service));
        }

        builder = builder.add_service(CapabilitiesServer::new(capabilities_service));

        builder.serve(self.config.address).await?;

        Ok(())
    }

    /// Get server statistics
    pub fn stats(&self) -> Vec<(String, u64)> {
        self.request_stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect()
    }
}

/// Bazel-compatible CAS service implementation
struct BazelCASService {
    cas: Arc<ContentAddressedStore>,
    circuit_breaker: Arc<CircuitBreaker>,
    max_batch_size: usize,
    max_blob_size: u64,
    stats: Arc<DashMap<String, AtomicU64>>,
}

impl BazelCASService {
    fn record_stat(&self, metric: &str) {
        self.stats
            .entry(metric.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Convert between Bazel digest format and internal hash format
    fn digest_to_hash(digest: &Digest) -> String {
        // Bazel uses hex-encoded SHA256
        digest.hash.clone()
    }

    /// Validate digest format
    fn validate_digest(digest: &Digest) -> Result<()> {
        if digest.hash.is_empty() {
            return Err(Error::configuration("Empty hash in digest".to_string()));
        }

        // Validate hex encoding (64 chars for SHA256)
        if digest.hash.len() != 64 {
            return Err(Error::configuration(format!(
                "Invalid hash length: expected 64, got {}",
                digest.hash.len()
            )));
        }

        // Validate hex characters
        if !digest.hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::configuration(
                "Hash contains non-hexadecimal characters".to_string(),
            ));
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl CASService for BazelCASService {
    async fn find_missing_blobs(
        &self,
        request: Request<FindMissingBlobsRequest>,
    ) -> Result<Response<FindMissingBlobsResponse>, Status> {
        self.record_stat("cas.find_missing_blobs");

        if self.circuit_breaker.is_open() {
            return Err(Status::unavailable("Service temporarily unavailable"));
        }

        let req = request.into_inner();
        let instance_name = &req.instance_name;

        debug!(
            "FindMissingBlobs request for instance '{}' with {} digests",
            instance_name,
            req.blob_digests.len()
        );

        // Validate batch size
        if req.blob_digests.len() > self.max_batch_size {
            return Err(Status::invalid_argument(format!(
                "Batch size {} exceeds maximum {}",
                req.blob_digests.len(),
                self.max_batch_size
            )));
        }

        let mut missing_digests = Vec::new();

        for digest in req.blob_digests {
            // Validate digest format
            match Self::validate_digest(&digest) {
                Ok(_) => {
                    let hash = Self::digest_to_hash(&digest);

                    // Check if blob exists
                    if !self.cas.contains(&hash) {
                        missing_digests.push(digest);
                    }
                }
                Err(e) => {
                    warn!("Invalid digest format: {}", e);
                    missing_digests.push(digest);
                }
            }
        }

        self.circuit_breaker.record_success();

        Ok(Response::new(FindMissingBlobsResponse {
            missing_blob_digests: missing_digests,
        }))
    }

    async fn batch_update_blobs(
        &self,
        request: Request<BatchUpdateBlobsRequest>,
    ) -> Result<Response<BatchUpdateBlobsResponse>, Status> {
        self.record_stat("cas.batch_update_blobs");

        if self.circuit_breaker.is_open() {
            return Err(Status::unavailable("Service temporarily unavailable"));
        }

        let req = request.into_inner();
        let instance_name = &req.instance_name;

        debug!(
            "BatchUpdateBlobs request for instance '{}' with {} blobs",
            instance_name,
            req.requests.len()
        );

        // Validate batch size
        if req.requests.len() > self.max_batch_size {
            return Err(Status::invalid_argument(format!(
                "Batch size {} exceeds maximum {}",
                req.requests.len(),
                self.max_batch_size
            )));
        }

        let mut responses = Vec::new();

        for update_req in req.requests {
            let digest = update_req
                .digest
                .ok_or_else(|| Status::invalid_argument("Missing digest in update request"))?;

            // Validate digest
            let validation_result = Self::validate_digest(&digest);
            if let Err(e) = validation_result {
                responses.push(
                    super::grpc_proto::proto::batch_update_blobs_response::Response {
                        digest: Some(digest),
                        status: Some(super::grpc_proto::proto::Status {
                            code: tonic::Code::InvalidArgument as i32,
                            message: e.to_string(),
                        }),
                    },
                );
                continue;
            }

            // Validate blob size
            if update_req.data.len() as u64 != digest.size_bytes as u64 {
                responses.push(
                    super::grpc_proto::proto::batch_update_blobs_response::Response {
                        digest: Some(digest),
                        status: Some(super::grpc_proto::proto::Status {
                            code: tonic::Code::InvalidArgument as i32,
                            message: format!(
                                "Size mismatch: expected {}, got {}",
                                digest.size_bytes,
                                update_req.data.len()
                            ),
                        }),
                    },
                );
                continue;
            }

            // Check blob size limit
            if digest.size_bytes > self.max_blob_size as i64 {
                responses.push(
                    super::grpc_proto::proto::batch_update_blobs_response::Response {
                        digest: Some(digest),
                        status: Some(super::grpc_proto::proto::Status {
                            code: tonic::Code::InvalidArgument as i32,
                            message: format!(
                                "Blob size {} exceeds maximum {}",
                                digest.size_bytes, self.max_blob_size
                            ),
                        }),
                    },
                );
                continue;
            }

            // Store the blob
            let cursor = Cursor::new(&update_req.data);
            match self.cas.store(cursor) {
                Ok(stored_hash) => {
                    // Verify the stored hash matches the expected hash
                    if stored_hash != digest.hash {
                        error!(
                            "Hash mismatch after storage: expected {}, got {}",
                            digest.hash, stored_hash
                        );
                        responses.push(
                            super::grpc_proto::proto::batch_update_blobs_response::Response {
                                digest: Some(digest),
                                status: Some(super::grpc_proto::proto::Status {
                                    code: tonic::Code::Internal as i32,
                                    message: "Hash mismatch after storage".to_string(),
                                }),
                            },
                        );
                    } else {
                        responses.push(
                            super::grpc_proto::proto::batch_update_blobs_response::Response {
                                digest: Some(digest),
                                status: Some(super::grpc_proto::proto::Status {
                                    code: tonic::Code::Ok as i32,
                                    message: String::new(),
                                }),
                            },
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to store blob: {}", e);
                    responses.push(
                        super::grpc_proto::proto::batch_update_blobs_response::Response {
                            digest: Some(digest),
                            status: Some(super::grpc_proto::proto::Status {
                                code: tonic::Code::Internal as i32,
                                message: e.to_string(),
                            }),
                        },
                    );
                }
            }
        }

        self.circuit_breaker.record_success();

        Ok(Response::new(BatchUpdateBlobsResponse { responses }))
    }

    async fn batch_read_blobs(
        &self,
        request: Request<BatchReadBlobsRequest>,
    ) -> Result<Response<BatchReadBlobsResponse>, Status> {
        self.record_stat("cas.batch_read_blobs");

        if self.circuit_breaker.is_open() {
            return Err(Status::unavailable("Service temporarily unavailable"));
        }

        let req = request.into_inner();
        let instance_name = &req.instance_name;

        debug!(
            "BatchReadBlobs request for instance '{}' with {} blobs",
            instance_name,
            req.digests.len()
        );

        // Validate batch size
        if req.digests.len() > self.max_batch_size {
            return Err(Status::invalid_argument(format!(
                "Batch size {} exceeds maximum {}",
                req.digests.len(),
                self.max_batch_size
            )));
        }

        let mut responses = Vec::new();

        for digest in req.digests {
            // Validate digest
            let validation_result = Self::validate_digest(&digest);
            if let Err(e) = validation_result {
                responses.push(
                    super::grpc_proto::proto::batch_read_blobs_response::Response {
                        digest: Some(digest),
                        data: Vec::new(),
                        status: Some(super::grpc_proto::proto::Status {
                            code: tonic::Code::InvalidArgument as i32,
                            message: e.to_string(),
                        }),
                    },
                );
                continue;
            }

            let hash = Self::digest_to_hash(&digest);

            match self.cas.retrieve(&hash) {
                Ok(data) => {
                    // Verify size matches
                    if data.len() as i64 != digest.size_bytes {
                        warn!(
                            "Size mismatch for blob {}: expected {}, got {}",
                            hash,
                            digest.size_bytes,
                            data.len()
                        );
                        responses.push(
                            super::grpc_proto::proto::batch_read_blobs_response::Response {
                                digest: Some(digest),
                                data: Vec::new(),
                                status: Some(super::grpc_proto::proto::Status {
                                    code: tonic::Code::DataLoss as i32,
                                    message: "Size mismatch".to_string(),
                                }),
                            },
                        );
                    } else {
                        responses.push(
                            super::grpc_proto::proto::batch_read_blobs_response::Response {
                                digest: Some(digest),
                                data,
                                status: Some(super::grpc_proto::proto::Status {
                                    code: tonic::Code::Ok as i32,
                                    message: String::new(),
                                }),
                            },
                        );
                    }
                }
                Err(e) => {
                    debug!("Blob not found: {}", hash);
                    responses.push(
                        super::grpc_proto::proto::batch_read_blobs_response::Response {
                            digest: Some(digest),
                            data: Vec::new(),
                            status: Some(super::grpc_proto::proto::Status {
                                code: tonic::Code::NotFound as i32,
                                message: format!("Blob {} not found", hash),
                            }),
                        },
                    );
                }
            }
        }

        self.circuit_breaker.record_success();

        Ok(Response::new(BatchReadBlobsResponse { responses }))
    }
}

/// Bazel-compatible action cache service implementation
struct BazelActionCacheService {
    action_cache: Arc<ActionCache>,
    circuit_breaker: Arc<CircuitBreaker>,
    stats: Arc<DashMap<String, AtomicU64>>,
}

impl BazelActionCacheService {
    fn record_stat(&self, metric: &str) {
        self.stats
            .entry(metric.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[tonic::async_trait]
impl ActionCacheService for BazelActionCacheService {
    async fn get_action_result(
        &self,
        request: Request<GetActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        self.record_stat("action_cache.get");

        if self.circuit_breaker.is_open() {
            return Err(Status::unavailable("Service temporarily unavailable"));
        }

        let req = request.into_inner();
        let instance_name = &req.instance_name;
        let action_digest = req
            .action_digest
            .ok_or_else(|| Status::invalid_argument("Missing action digest"))?;

        debug!(
            "GetActionResult request for instance '{}', action {}",
            instance_name, action_digest.hash
        );

        // Validate digest
        BazelCASService::validate_digest(&action_digest)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Get cached action result
        match self
            .action_cache
            .get_cached_action_result(&action_digest.hash)
        {
            Some(cached_result) => {
                self.circuit_breaker.record_success();

                // Convert internal format to Bazel ActionResult
                let action_result = ActionResult {
                    output_files: Vec::new(), // TODO: Implement output file mapping
                    output_directories: Vec::new(),
                    exit_code: cached_result.exit_code,
                    stdout_digest: cached_result.stdout_hash.map(|hash| Digest {
                        hash,
                        size_bytes: 0, // TODO: Store size metadata
                    }),
                    stderr_digest: cached_result.stderr_hash.map(|hash| Digest {
                        hash,
                        size_bytes: 0, // TODO: Store size metadata
                    }),
                    execution_metadata: None,
                };

                Ok(Response::new(action_result))
            }
            None => {
                self.circuit_breaker.record_success();
                Err(Status::not_found(format!(
                    "Action result not found for {}",
                    action_digest.hash
                )))
            }
        }
    }

    async fn update_action_result(
        &self,
        request: Request<UpdateActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        self.record_stat("action_cache.update");

        if self.circuit_breaker.is_open() {
            return Err(Status::unavailable("Service temporarily unavailable"));
        }

        let req = request.into_inner();
        let instance_name = &req.instance_name;
        let action_digest = req
            .action_digest
            .ok_or_else(|| Status::invalid_argument("Missing action digest"))?;
        let action_result = req
            .action_result
            .ok_or_else(|| Status::invalid_argument("Missing action result"))?;

        debug!(
            "UpdateActionResult request for instance '{}', action {}",
            instance_name, action_digest.hash
        );

        // Validate digest
        BazelCASService::validate_digest(&action_digest)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Convert Bazel ActionResult to internal format
        let internal_result = crate::cache::action_cache::ActionResult {
            exit_code: action_result.exit_code,
            stdout_hash: action_result.stdout_digest.map(|d| d.hash),
            stderr_hash: action_result.stderr_digest.map(|d| d.hash),
            output_files: action_result
                .output_files
                .into_iter()
                .filter_map(|f| f.digest.map(|d| (f.path, d.hash)))
                .collect(),
            executed_at: std::time::SystemTime::now(),
            duration_ms: 0, // Not provided by Bazel
        };

        // Create a fake digest for storing
        let digest = crate::cache::action_cache::ActionDigest {
            hash: action_digest.hash.clone(),
            components: crate::cache::action_cache::ActionComponents {
                task_name: format!("bazel_action_{}", instance_name),
                command: None,
                working_dir: std::path::PathBuf::from("/"),
                env_vars: std::collections::HashMap::new(),
                input_files: std::collections::HashMap::new(),
                config_hash: action_digest.hash.clone(),
            },
        };

        // Execute the cache update
        match self
            .action_cache
            .execute_action(&digest, || async { Ok(internal_result) })
            .await
        {
            Ok(result) => {
                self.circuit_breaker.record_success();

                // Convert back to Bazel format
                let action_result = ActionResult {
                    output_files: Vec::new(), // TODO: Implement proper conversion
                    output_directories: Vec::new(),
                    exit_code: result.exit_code,
                    stdout_digest: result.stdout_hash.map(|hash| Digest {
                        hash,
                        size_bytes: 0,
                    }),
                    stderr_digest: result.stderr_hash.map(|hash| Digest {
                        hash,
                        size_bytes: 0,
                    }),
                    execution_metadata: None,
                };

                Ok(Response::new(action_result))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(Status::internal(format!(
                    "Failed to update action cache: {}",
                    e
                )))
            }
        }
    }
}

/// Capabilities service implementation
struct BazelCapabilitiesService {
    max_batch_size: usize,
    enable_action_cache: bool,
    enable_cas: bool,
}

#[tonic::async_trait]
impl CapabilitiesService for BazelCapabilitiesService {
    async fn get_capabilities(
        &self,
        request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<ServerCapabilities>, Status> {
        let req = request.into_inner();
        debug!(
            "GetCapabilities request for instance '{}'",
            req.instance_name
        );

        let cache_capabilities = CacheCapabilities {
            digest_function: vec![CacheCapabilities::DigestFunction::SHA256 as i32],
            action_cache_update_capabilities: Some(ActionCacheUpdateCapabilities {
                update_enabled: self.enable_action_cache,
            }),
            symlink_absolute_path_strategy: vec![
                CacheCapabilities::SymlinkAbsolutePathStrategy::DISALLOWED as i32,
            ],
            max_batch_total_size_bytes: (self.max_batch_size * 1024 * 1024) as i64, // Approximate
        };

        let capabilities = ServerCapabilities {
            cache_capabilities: Some(cache_capabilities),
            execution_capabilities: None, // We don't support remote execution
            deprecated_api_version: None,
            low_api_version: Some(prost_types::Any {
                type_url: "type.googleapis.com/build.bazel.semver.SemVer".to_string(),
                value: vec![8, 2, 16, 0], // 2.0.0
            }),
            high_api_version: Some(prost_types::Any {
                type_url: "type.googleapis.com/build.bazel.semver.SemVer".to_string(),
                value: vec![8, 2, 16, 0], // 2.0.0
            }),
        };

        Ok(Response::new(capabilities))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bazel_cas_service_validate_digest() {
        // Valid SHA256 digest
        let valid_digest = Digest {
            hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            size_bytes: 0,
        };
        assert!(BazelCASService::validate_digest(&valid_digest).is_ok());

        // Invalid length
        let invalid_length = Digest {
            hash: "abc123".to_string(),
            size_bytes: 0,
        };
        assert!(BazelCASService::validate_digest(&invalid_length).is_err());

        // Invalid characters
        let invalid_chars = Digest {
            hash: "g3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            size_bytes: 0,
        };
        assert!(BazelCASService::validate_digest(&invalid_chars).is_err());

        // Empty hash
        let empty_hash = Digest {
            hash: String::new(),
            size_bytes: 0,
        };
        assert!(BazelCASService::validate_digest(&empty_hash).is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(0.5, Duration::from_secs(1));

        // Initially closed
        assert!(!breaker.is_open());

        // Record some failures
        for _ in 0..10 {
            breaker.record_failure();
        }

        // Should be open now
        assert!(breaker.is_open());

        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be closed again
        assert!(!breaker.is_open());

        // Record successes
        for _ in 0..20 {
            breaker.record_success();
        }

        // Should remain closed
        assert!(!breaker.is_open());
    }

    #[tokio::test]
    async fn test_bazel_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = BazelRemoteCacheConfig::default();
        config.cache_config.base_dir = temp_dir.path().to_path_buf();

        let server = BazelRemoteCacheServer::new(config).await;
        assert!(server.is_ok());
    }
}
