//! Remote cache server implementation for Bazel/Buck2 compatibility
use anyhow::Result;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, info, warn};

use super::grpc_proto::proto::{
    action_cache_server::{ActionCache as ActionCacheService, ActionCacheServer},
    capabilities_server::{Capabilities as CapabilitiesService, CapabilitiesServer},
    content_addressable_storage_server::{
        ContentAddressableStorage as CASService, ContentAddressableStorageServer,
    },
    ActionResult, BatchReadBlobsRequest, BatchReadBlobsResponse, BatchUpdateBlobsRequest,
    BatchUpdateBlobsResponse, Digest, FindMissingBlobsRequest, FindMissingBlobsResponse,
    GetActionResultRequest, GetCapabilitiesRequest, ServerCapabilities, UpdateActionResultRequest,
};
use crate::cache::CacheManager as CuenvCacheManager;
use crate::cache::{ActionCache, CacheConfig, ContentAddressedStore};

/// Configuration for the remote cache server
pub struct RemoteCacheConfig {
    pub address: SocketAddr,
    pub enable_action_cache: bool,
    pub enable_cas: bool,
    pub cache_config: CacheConfig,
}

/// Remote cache server that implements Bazel's Remote Execution API
pub struct RemoteCacheServer {
    address: SocketAddr,
    cache_manager: Arc<CuenvCacheManager>,
    cas: Arc<ContentAddressedStore>,
    action_cache: Arc<ActionCache>,
}

impl RemoteCacheServer {
    /// Create a new remote cache server
    pub async fn new(config: RemoteCacheConfig) -> Result<Self> {
        let cache_manager = Arc::new(CuenvCacheManager::new(config.cache_config).await?);
        let cas = cache_manager.content_store();
        let action_cache = cache_manager.action_cache();

        Ok(Self {
            address: config.address,
            cache_manager,
            cas,
            action_cache,
        })
    }

    /// Start serving the remote cache
    pub async fn serve(self) -> Result<()> {
        let cas_service = RemoteCASService {
            cas: Arc::clone(&self.cas),
        };
        let action_cache_service = RemoteActionCacheService {
            action_cache: Arc::clone(&self.action_cache),
        };
        let capabilities_service = RemoteCapabilitiesService::new();

        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(super::grpc_proto::proto::FILE_DESCRIPTOR_SET)
            .build()?;

        info!("Starting remote cache server on {}", self.address);

        Server::builder()
            .add_service(ContentAddressableStorageServer::new(cas_service))
            .add_service(ActionCacheServer::new(action_cache_service))
            .add_service(CapabilitiesServer::new(capabilities_service))
            .add_service(reflection_service)
            .serve(self.address)
            .await?;

        Ok(())
    }
}

/// Content-Addressable Storage service implementation
struct RemoteCASService {
    cas: Arc<ContentAddressedStore>,
}

#[async_trait]
impl CASService for RemoteCASService {
    async fn find_missing_blobs(
        &self,
        request: Request<FindMissingBlobsRequest>,
    ) -> Result<Response<FindMissingBlobsResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "FindMissingBlobs request for {} digests",
            req.blob_digests.len()
        );

        let mut missing_digests = Vec::new();
        for digest in req.blob_digests {
            let hash = hex::encode(&digest.hash);
            if !self.cas.contains(&hash).await {
                missing_digests.push(digest);
            }
        }

        Ok(Response::new(FindMissingBlobsResponse {
            missing_blob_digests: missing_digests,
        }))
    }

    async fn batch_update_blobs(
        &self,
        request: Request<BatchUpdateBlobsRequest>,
    ) -> Result<Response<BatchUpdateBlobsResponse>, Status> {
        let req = request.into_inner();
        debug!("BatchUpdateBlobs request for {} blobs", req.requests.len());

        let mut responses = Vec::new();
        for update_req in req.requests {
            let digest = update_req
                .digest
                .ok_or_else(|| Status::invalid_argument("Missing digest in update request"))?;

            let hash = hex::encode(&digest.hash);
            match self.cas.store(&hash, &update_req.data).await {
                Ok(_) => {
                    responses.push(BatchUpdateBlobsResponse::Response {
                        digest: Some(digest),
                        status: Some(Default::default()), // OK status
                    });
                }
                Err(e) => {
                    warn!("Failed to store blob {}: {}", hash, e);
                    responses.push(BatchUpdateBlobsResponse::Response {
                        digest: Some(digest),
                        status: Some(rpc_status::Status {
                            code: tonic::Code::Internal as i32,
                            message: e.to_string(),
                            ..Default::default()
                        }),
                    });
                }
            }
        }

        Ok(Response::new(BatchUpdateBlobsResponse { responses }))
    }

    async fn batch_read_blobs(
        &self,
        request: Request<BatchReadBlobsRequest>,
    ) -> Result<Response<BatchReadBlobsResponse>, Status> {
        let req = request.into_inner();
        debug!("BatchReadBlobs request for {} blobs", req.digests.len());

        let mut responses = Vec::new();
        for digest in req.digests {
            let hash = hex::encode(&digest.hash);
            match self.cas.retrieve(&hash).await {
                Ok(Some(data)) => {
                    responses.push(BatchReadBlobsResponse::Response {
                        digest: Some(digest),
                        data,
                        status: Some(Default::default()), // OK status
                    });
                }
                Ok(None) => {
                    responses.push(BatchReadBlobsResponse::Response {
                        digest: Some(digest),
                        data: Vec::new(),
                        status: Some(rpc_status::Status {
                            code: tonic::Code::NotFound as i32,
                            message: format!("Blob {} not found", hash),
                            ..Default::default()
                        }),
                    });
                }
                Err(e) => {
                    warn!("Failed to retrieve blob {}: {}", hash, e);
                    responses.push(BatchReadBlobsResponse::Response {
                        digest: Some(digest),
                        data: Vec::new(),
                        status: Some(rpc_status::Status {
                            code: tonic::Code::Internal as i32,
                            message: e.to_string(),
                            ..Default::default()
                        }),
                    });
                }
            }
        }

        Ok(Response::new(BatchReadBlobsResponse { responses }))
    }

    // Other methods would be implemented similarly...
}

/// Action Cache service implementation
struct RemoteActionCacheService {
    action_cache: Arc<ActionCache>,
}

#[async_trait]
impl ActionCacheService for RemoteActionCacheService {
    async fn get_action_result(
        &self,
        request: Request<GetActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        let req = request.into_inner();
        let digest = req
            .action_digest
            .ok_or_else(|| Status::invalid_argument("Missing action digest"))?;

        let hash = hex::encode(&digest.hash);
        debug!("GetActionResult for digest {}", hash);

        match self.action_cache.get(&hash).await {
            Ok(Some(result)) => {
                // Convert internal format to proto ActionResult
                let action_result = ActionResult {
                    output_files: vec![], // Would be populated from cached data
                    output_directories: vec![],
                    exit_code: 0,
                    stdout_digest: None,
                    stderr_digest: None,
                    // ... other fields
                    ..Default::default()
                };
                Ok(Response::new(action_result))
            }
            Ok(None) => Err(Status::not_found(format!(
                "Action result for {} not found",
                hash
            ))),
            Err(e) => {
                warn!("Failed to get action result: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    async fn update_action_result(
        &self,
        request: Request<UpdateActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        let req = request.into_inner();
        let digest = req
            .action_digest
            .ok_or_else(|| Status::invalid_argument("Missing action digest"))?;
        let action_result = req
            .action_result
            .ok_or_else(|| Status::invalid_argument("Missing action result"))?;

        let hash = hex::encode(&digest.hash);
        debug!("UpdateActionResult for digest {}", hash);

        // Store the action result
        match self.action_cache.put(&hash, &action_result).await {
            Ok(_) => Ok(Response::new(action_result)),
            Err(e) => {
                warn!("Failed to update action result: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
}

/// Capabilities service implementation
struct RemoteCapabilitiesService;

impl RemoteCapabilitiesService {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CapabilitiesService for RemoteCapabilitiesService {
    async fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<ServerCapabilities>, Status> {
        Ok(Response::new(ServerCapabilities {
            cache_capabilities: Some(Default::default()),
            execution_capabilities: None, // We only provide caching
            deprecated_api_version: None,
            low_api_version: Some(Default::default()),
            high_api_version: Some(Default::default()),
        }))
    }
}
