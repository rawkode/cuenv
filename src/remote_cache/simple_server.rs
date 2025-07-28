//! Simplified remote cache server implementation
use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, info};

use super::grpc_proto::proto::{
    action_cache_server::{ActionCache as ActionCacheService, ActionCacheServer},
    capabilities_server::{Capabilities as CapabilitiesService, CapabilitiesServer},
    content_addressable_storage_server::{
        ContentAddressableStorage as CASService, ContentAddressableStorageServer,
    },
    ActionResult, BatchReadBlobsRequest, BatchReadBlobsResponse, BatchUpdateBlobsRequest,
    BatchUpdateBlobsResponse, FindMissingBlobsRequest, FindMissingBlobsResponse,
    GetActionResultRequest, GetCapabilitiesRequest, ServerCapabilities, UpdateActionResultRequest,
};
use crate::cache::{CacheConfig, ContentAddressedStore};

/// Configuration for the remote cache server
pub struct RemoteCacheConfig {
    pub address: SocketAddr,
    pub enable_action_cache: bool,
    pub enable_cas: bool,
    pub cache_config: CacheConfig,
}

/// Remote cache server
pub struct RemoteCacheServer {
    address: SocketAddr,
    cas: Arc<ContentAddressedStore>,
}

impl RemoteCacheServer {
    /// Create a new remote cache server
    pub async fn new(config: RemoteCacheConfig) -> Result<Self> {
        // Create cache directories
        std::fs::create_dir_all(&config.cache_config.base_dir)?;
        let cas_dir = config.cache_config.base_dir.join("cas");
        std::fs::create_dir_all(&cas_dir)?;

        // Initialize content-addressed store
        let cas = Arc::new(ContentAddressedStore::new(
            cas_dir,
            config.cache_config.inline_threshold,
        )?);

        Ok(Self {
            address: config.address,
            cas,
        })
    }

    /// Start serving the remote cache
    pub async fn serve(self) -> Result<()> {
        let cas_service = SimpleCASService {
            cas: Arc::clone(&self.cas),
        };
        let action_cache_service = SimpleActionCacheService;
        let capabilities_service = SimpleCapabilitiesService;

        info!("Starting remote cache server on {}", self.address);

        Server::builder()
            .add_service(ContentAddressableStorageServer::new(cas_service))
            .add_service(ActionCacheServer::new(action_cache_service))
            .add_service(CapabilitiesServer::new(capabilities_service))
            .serve(self.address)
            .await?;

        Ok(())
    }
}

/// Simplified CAS service
struct SimpleCASService {
    cas: Arc<ContentAddressedStore>,
}

#[tonic::async_trait]
impl CASService for SimpleCASService {
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
            // Check if blob exists by trying to retrieve it
            if self.cas.retrieve(&hash).is_err() {
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

            use std::io::Cursor;
            let cursor = Cursor::new(&update_req.data);
            match self.cas.store(cursor) {
                Ok(_stored_hash) => {
                    responses.push(
                        super::grpc_proto::proto::batch_update_blobs_response::Response {
                            digest: Some(digest),
                            status: Some(Default::default()), // OK status
                        },
                    );
                }
                Err(e) => {
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
            match self.cas.retrieve(&hash) {
                Ok(data) => {
                    responses.push(
                        super::grpc_proto::proto::batch_read_blobs_response::Response {
                            digest: Some(digest),
                            data,
                            status: Some(Default::default()), // OK status
                        },
                    );
                }
                Err(_) => {
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

        Ok(Response::new(BatchReadBlobsResponse { responses }))
    }
}

/// Simplified action cache service (no-op for now)
struct SimpleActionCacheService;

#[tonic::async_trait]
impl ActionCacheService for SimpleActionCacheService {
    async fn get_action_result(
        &self,
        _request: Request<GetActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        Err(Status::not_found("Action cache not implemented"))
    }

    async fn update_action_result(
        &self,
        _request: Request<UpdateActionResultRequest>,
    ) -> Result<Response<ActionResult>, Status> {
        Err(Status::unimplemented("Action cache not implemented"))
    }
}

/// Capabilities service
struct SimpleCapabilitiesService;

#[tonic::async_trait]
impl CapabilitiesService for SimpleCapabilitiesService {
    async fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<ServerCapabilities>, Status> {
        Ok(Response::new(ServerCapabilities {
            cache_capabilities: Some(Default::default()),
            execution_capabilities: None,
            deprecated_api_version: None,
            low_api_version: Some(Default::default()),
            high_api_version: Some(Default::default()),
        }))
    }
}
