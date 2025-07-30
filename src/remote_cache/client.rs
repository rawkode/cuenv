//! Remote cache client for integration with Bazel-compatible cache servers
//!
//! This module provides a client implementation that can connect to remote
//! cache servers implementing the Bazel Remote Execution API.

use crate::errors::{Error, Result};
use anyhow::Result as AnyhowResult;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};
use tracing::{debug, error, info, warn};

use super::grpc_proto::proto::{
    action_cache_client::ActionCacheClient, capabilities_client::CapabilitiesClient,
    content_addressable_storage_client::ContentAddressableStorageClient, ActionResult,
    BatchReadBlobsRequest, BatchUpdateBlobsRequest, Digest, FindMissingBlobsRequest,
    GetActionResultRequest, GetCapabilitiesRequest, UpdateActionResultRequest,
};

/// Configuration for the remote cache client
#[derive(Debug, Clone)]
pub struct RemoteCacheClientConfig {
    /// Remote cache server address (e.g., "grpc://localhost:50051")
    pub server_address: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry backoff base duration
    pub retry_backoff: Duration,
    /// Enable TLS
    pub use_tls: bool,
    /// Maximum batch size for batch operations
    pub max_batch_size: usize,
    /// Circuit breaker failure threshold (0.0 - 1.0)
    pub circuit_breaker_threshold: f64,
    /// Circuit breaker recovery timeout
    pub circuit_breaker_timeout: Duration,
}

impl Default for RemoteCacheClientConfig {
    fn default() -> Self {
        Self {
            server_address: "grpc://localhost:50051".to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: Duration::from_millis(100),
            use_tls: false,
            max_batch_size: 1000,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for fault tolerance
struct CircuitBreaker {
    state: RwLock<CircuitBreakerState>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: RwLock<Option<Instant>>,
    consecutive_successes: AtomicU64,
    threshold: f64,
    timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: f64, timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitBreakerState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            consecutive_successes: AtomicU64::new(0),
            threshold,
            timeout,
        }
    }

    fn can_proceed(&self) -> bool {
        let state = *self.state.read();
        match state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = *self.last_failure_time.read() {
                    if last_failure.elapsed() >= self.timeout {
                        *self.state.write() = CircuitBreakerState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.fetch_add(1, Ordering::Relaxed);

        let state = *self.state.read();
        match state {
            CircuitBreakerState::HalfOpen => {
                // Transition back to closed after successful requests in half-open state
                if self.consecutive_successes.load(Ordering::Relaxed) >= 3 {
                    *self.state.write() = CircuitBreakerState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.consecutive_successes.store(0, Ordering::Relaxed);
                    info!("Circuit breaker transitioned to CLOSED state");
                }
            }
            _ => {}
        }
    }

    fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        *self.last_failure_time.write() = Some(Instant::now());

        let total = self.failure_count.load(Ordering::Relaxed) as f64
            + self.success_count.load(Ordering::Relaxed) as f64;

        if total >= 10.0 {
            let failure_rate = self.failure_count.load(Ordering::Relaxed) as f64 / total;
            if failure_rate > self.threshold {
                let current_state = *self.state.read();
                if current_state != CircuitBreakerState::Open {
                    *self.state.write() = CircuitBreakerState::Open;
                    warn!(
                        "Circuit breaker transitioned to OPEN state (failure rate: {:.2}%)",
                        failure_rate * 100.0
                    );
                }
            }
        }
    }
}

/// Remote cache client
pub struct RemoteCacheClient {
    config: RemoteCacheClientConfig,
    cas_client: ContentAddressableStorageClient<Channel>,
    action_cache_client: ActionCacheClient<Channel>,
    capabilities_client: CapabilitiesClient<Channel>,
    circuit_breaker: Arc<CircuitBreaker>,
    request_stats: Arc<DashMap<String, AtomicU64>>,
}

impl RemoteCacheClient {
    /// Create a new remote cache client
    pub async fn new(config: RemoteCacheClientConfig) -> AnyhowResult<Self> {
        // Parse and validate server address
        let endpoint = if config.server_address.starts_with("grpc://") {
            let address = config.server_address.strip_prefix("grpc://").unwrap();
            Endpoint::from_shared(format!("http://{}", address))?
        } else if config.server_address.starts_with("grpcs://") {
            let address = config.server_address.strip_prefix("grpcs://").unwrap();
            Endpoint::from_shared(format!("https://{}", address))?
                .tls_config(tonic::transport::ClientTlsConfig::new())?
        } else {
            return Err(anyhow::anyhow!(
                "Invalid server address format. Use grpc:// or grpcs://"
            ));
        };

        let endpoint = endpoint
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout);

        info!(
            "Connecting to remote cache server: {}",
            config.server_address
        );
        let channel = endpoint.connect().await?;

        let cas_client = ContentAddressableStorageClient::new(channel.clone());
        let action_cache_client = ActionCacheClient::new(channel.clone());
        let capabilities_client = CapabilitiesClient::new(channel);

        let circuit_breaker = Arc::new(CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_timeout,
        ));

        Ok(Self {
            config,
            cas_client,
            action_cache_client,
            capabilities_client,
            circuit_breaker,
            request_stats: Arc::new(DashMap::new()),
        })
    }

    /// Get server capabilities
    pub async fn get_capabilities(
        &mut self,
        instance_name: &str,
    ) -> Result<super::grpc_proto::proto::ServerCapabilities> {
        self.record_stat("capabilities.get");

        if !self.circuit_breaker.can_proceed() {
            return Err(Error::configuration(
                "Circuit breaker is open - remote cache unavailable".to_string(),
            ));
        }

        let request = Request::new(GetCapabilitiesRequest {
            instance_name: instance_name.to_string(),
        });

        match self.capabilities_client.get_capabilities(request).await {
            Ok(response) => {
                self.circuit_breaker.record_success();
                Ok(response.into_inner())
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(Error::configuration(format!(
                    "Failed to get capabilities: {}",
                    e
                )))
            }
        }
    }

    /// Check which blobs are missing from the remote cache
    pub async fn find_missing_blobs(
        &mut self,
        instance_name: &str,
        digests: Vec<Digest>,
    ) -> Result<Vec<Digest>> {
        self.record_stat("cas.find_missing_blobs");

        if !self.circuit_breaker.can_proceed() {
            return Err(Error::configuration(
                "Circuit breaker is open - remote cache unavailable".to_string(),
            ));
        }

        // Split into batches if necessary
        let mut all_missing = Vec::new();

        for chunk in digests.chunks(self.config.max_batch_size) {
            let request = Request::new(FindMissingBlobsRequest {
                instance_name: instance_name.to_string(),
                blob_digests: chunk.to_vec(),
            });

            match self
                .with_retry(|| async { self.cas_client.find_missing_blobs(request.clone()).await })
                .await
            {
                Ok(response) => {
                    self.circuit_breaker.record_success();
                    all_missing.extend(response.into_inner().missing_blob_digests);
                }
                Err(e) => {
                    self.circuit_breaker.record_failure();
                    return Err(Error::configuration(format!(
                        "Failed to find missing blobs: {}",
                        e
                    )));
                }
            }
        }

        Ok(all_missing)
    }

    /// Upload blobs to the remote cache
    pub async fn upload_blobs(
        &mut self,
        instance_name: &str,
        blobs: Vec<(Digest, Vec<u8>)>,
    ) -> Result<Vec<(Digest, bool)>> {
        self.record_stat("cas.upload_blobs");

        if !self.circuit_breaker.can_proceed() {
            return Err(Error::configuration(
                "Circuit breaker is open - remote cache unavailable".to_string(),
            ));
        }

        let mut results = Vec::new();

        // Split into batches
        for chunk in blobs.chunks(self.config.max_batch_size) {
            let requests = chunk
                .iter()
                .map(|(digest, data)| {
                    super::grpc_proto::proto::batch_update_blobs_request::Request {
                        digest: Some(digest.clone()),
                        data: data.clone(),
                    }
                })
                .collect();

            let request = Request::new(BatchUpdateBlobsRequest {
                instance_name: instance_name.to_string(),
                requests,
            });

            match self
                .with_retry(|| async { self.cas_client.batch_update_blobs(request.clone()).await })
                .await
            {
                Ok(response) => {
                    self.circuit_breaker.record_success();
                    for resp in response.into_inner().responses {
                        if let Some(digest) = resp.digest {
                            let success = resp.status.map_or(false, |s| s.code == 0);
                            results.push((digest, success));
                        }
                    }
                }
                Err(e) => {
                    self.circuit_breaker.record_failure();
                    // Add failures for remaining blobs
                    for (digest, _) in chunk {
                        results.push((digest.clone(), false));
                    }
                    error!("Failed to upload blobs: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// Download blobs from the remote cache
    pub async fn download_blobs(
        &mut self,
        instance_name: &str,
        digests: Vec<Digest>,
    ) -> Result<Vec<(Digest, Option<Vec<u8>>)>> {
        self.record_stat("cas.download_blobs");

        if !self.circuit_breaker.can_proceed() {
            return Err(Error::configuration(
                "Circuit breaker is open - remote cache unavailable".to_string(),
            ));
        }

        let mut results = Vec::new();

        // Split into batches
        for chunk in digests.chunks(self.config.max_batch_size) {
            let request = Request::new(BatchReadBlobsRequest {
                instance_name: instance_name.to_string(),
                digests: chunk.to_vec(),
            });

            match self
                .with_retry(|| async { self.cas_client.batch_read_blobs(request.clone()).await })
                .await
            {
                Ok(response) => {
                    self.circuit_breaker.record_success();
                    for resp in response.into_inner().responses {
                        if let Some(digest) = resp.digest {
                            let data = if resp.status.as_ref().map_or(false, |s| s.code == 0) {
                                Some(resp.data)
                            } else {
                                None
                            };
                            results.push((digest, data));
                        }
                    }
                }
                Err(e) => {
                    self.circuit_breaker.record_failure();
                    // Add None for remaining digests
                    for digest in chunk {
                        results.push((digest.clone(), None));
                    }
                    error!("Failed to download blobs: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// Get action result from remote cache
    pub async fn get_action_result(
        &mut self,
        instance_name: &str,
        action_digest: Digest,
    ) -> Result<Option<ActionResult>> {
        self.record_stat("action_cache.get");

        if !self.circuit_breaker.can_proceed() {
            return Ok(None); // Fail open for reads
        }

        let request = Request::new(GetActionResultRequest {
            instance_name: instance_name.to_string(),
            action_digest: Some(action_digest),
        });

        match self
            .with_retry(|| async {
                self.action_cache_client
                    .get_action_result(request.clone())
                    .await
            })
            .await
        {
            Ok(response) => {
                self.circuit_breaker.record_success();
                Ok(Some(response.into_inner()))
            }
            Err(status) => {
                if matches!(status.code(), tonic::Code::NotFound) {
                    self.circuit_breaker.record_success();
                    Ok(None)
                } else {
                    self.circuit_breaker.record_failure();
                    warn!("Failed to get action result: {}", status);
                    Ok(None) // Fail open for reads
                }
            }
        }
    }

    /// Update action result in remote cache
    pub async fn update_action_result(
        &mut self,
        instance_name: &str,
        action_digest: Digest,
        action_result: ActionResult,
    ) -> Result<()> {
        self.record_stat("action_cache.update");

        if !self.circuit_breaker.can_proceed() {
            // Fail silently for writes when circuit is open
            return Ok(());
        }

        let request = Request::new(UpdateActionResultRequest {
            instance_name: instance_name.to_string(),
            action_digest: Some(action_digest),
            action_result: Some(action_result),
        });

        match self
            .with_retry(|| async {
                self.action_cache_client
                    .update_action_result(request.clone())
                    .await
            })
            .await
        {
            Ok(_) => {
                self.circuit_breaker.record_success();
                Ok(())
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                warn!("Failed to update action result: {}", e);
                Ok(()) // Fail silently for writes
            }
        }
    }

    /// Execute a function with retry logic
    async fn with_retry<F, Fut, T>(&self, mut f: F) -> std::result::Result<T, Status>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<tonic::Response<T>, Status>>,
    {
        let mut last_error = None;
        let mut backoff = self.config.retry_backoff;

        for attempt in 0..=self.config.max_retries {
            match f().await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);

                    // Don't retry on certain error codes
                    if let Some(ref error) = last_error {
                        match error.code() {
                            tonic::Code::InvalidArgument
                            | tonic::Code::NotFound
                            | tonic::Code::AlreadyExists
                            | tonic::Code::PermissionDenied
                            | tonic::Code::Unauthenticated => {
                                return Err(error.clone());
                            }
                            _ => {}
                        }
                    }

                    if attempt < self.config.max_retries {
                        debug!(
                            "Request failed (attempt {}/{}), retrying in {:?}",
                            attempt + 1,
                            self.config.max_retries,
                            backoff
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2; // Exponential backoff
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Record a statistic
    fn record_stat(&self, metric: &str) {
        self.request_stats
            .entry(metric.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Get client statistics
    pub fn stats(&self) -> Vec<(String, u64)> {
        self.request_stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect()
    }

    /// Get circuit breaker state
    pub fn circuit_breaker_state(&self) -> String {
        let state = *self.circuit_breaker.state.read();
        format!("{:?}", state)
    }
}

/// Builder for RemoteCacheClient
pub struct RemoteCacheClientBuilder {
    config: RemoteCacheClientConfig,
}

impl RemoteCacheClientBuilder {
    /// Create a new builder
    pub fn new(server_address: &str) -> Self {
        let mut config = RemoteCacheClientConfig::default();
        config.server_address = server_address.to_string();
        Self { config }
    }

    /// Set connection timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set request timeout
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Set maximum retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Set retry backoff
    pub fn retry_backoff(mut self, backoff: Duration) -> Self {
        self.config.retry_backoff = backoff;
        self
    }

    /// Enable TLS
    pub fn use_tls(mut self, use_tls: bool) -> Self {
        self.config.use_tls = use_tls;
        self
    }

    /// Set maximum batch size
    pub fn max_batch_size(mut self, size: usize) -> Self {
        self.config.max_batch_size = size;
        self
    }

    /// Set circuit breaker threshold
    pub fn circuit_breaker_threshold(mut self, threshold: f64) -> Self {
        self.config.circuit_breaker_threshold = threshold;
        self
    }

    /// Set circuit breaker timeout
    pub fn circuit_breaker_timeout(mut self, timeout: Duration) -> Self {
        self.config.circuit_breaker_timeout = timeout;
        self
    }

    /// Build the client
    pub async fn build(self) -> AnyhowResult<RemoteCacheClient> {
        RemoteCacheClient::new(self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_state_transitions() {
        let breaker = CircuitBreaker::new(0.5, Duration::from_millis(100));

        // Initially closed
        assert!(breaker.can_proceed());

        // Record failures to open the circuit
        for _ in 0..10 {
            breaker.record_failure();
        }

        // Should be open
        assert!(!breaker.can_proceed());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        // Should allow one request (half-open)
        assert!(breaker.can_proceed());

        // Record successes to close the circuit
        for _ in 0..3 {
            breaker.record_success();
        }

        // Should be closed
        assert!(breaker.can_proceed());
    }

    #[test]
    fn test_remote_cache_client_builder() {
        let builder = RemoteCacheClientBuilder::new("grpc://localhost:50051")
            .connect_timeout(Duration::from_secs(5))
            .request_timeout(Duration::from_secs(10))
            .max_retries(5)
            .max_batch_size(500)
            .circuit_breaker_threshold(0.6);

        assert_eq!(builder.config.server_address, "grpc://localhost:50051");
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.config.request_timeout, Duration::from_secs(10));
        assert_eq!(builder.config.max_retries, 5);
        assert_eq!(builder.config.max_batch_size, 500);
        assert_eq!(builder.config.circuit_breaker_threshold, 0.6);
    }
}
