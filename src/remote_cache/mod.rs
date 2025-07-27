//! Remote Execution API compatible cache system for cuenv
//!
//! This module implements a Bazel/Buck2-style remote execution cache that
//! follows Google's Remote Execution API specification. It provides:
//!
//! - Action digest computation with hermetic input tracking
//! - Content-addressed storage with deduplication
//! - Remote cache client supporting HTTP/gRPC backends
//! - Sandboxed execution environment
//! - Comprehensive monitoring and statistics

pub mod action_digest;
pub mod cache_client;
pub mod cas_client;
pub mod remote_executor;
pub mod sandbox;
pub mod grpc_proto;
pub mod server;

// Re-export main types
pub use action_digest::{ActionDigest, DigestFunction};
pub use cache_client::{CacheClient, CacheClientConfig, RemoteBackend};
pub use cas_client::{CASClient, CASClientConfig, Digest};
pub use remote_executor::{RemoteExecutor, RemoteExecutorConfig};
pub use sandbox::{Sandbox, SandboxConfig, SandboxMode};
pub use server::{RemoteCacheServer, RemoteCacheConfig};

// Protocol types matching Remote Execution API
pub mod proto {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// Digest as defined in the Remote Execution API
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub struct Digest {
        /// The hash of the content
        pub hash: String,
        /// The size of the content in bytes
        pub size_bytes: i64,
    }

    /// Action specification
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Action {
        /// The command to execute
        pub command_digest: Digest,
        /// The input root digest
        pub input_root_digest: Digest,
        /// Timeout for the action
        pub timeout: Option<std::time::Duration>,
        /// Whether to cache the result
        pub do_not_cache: bool,
        /// Platform properties
        pub platform: Platform,
    }

    /// Command specification
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Command {
        /// Arguments to execute
        pub arguments: Vec<String>,
        /// Environment variables
        pub environment_variables: Vec<EnvironmentVariable>,
        /// Output files to capture
        pub output_files: Vec<String>,
        /// Output directories to capture
        pub output_directories: Vec<String>,
        /// Platform requirements
        pub platform: Platform,
        /// Working directory (relative to input root)
        pub working_directory: String,
    }

    /// Environment variable
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EnvironmentVariable {
        pub name: String,
        pub value: String,
    }

    /// Platform properties
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct Platform {
        /// Properties like OS, architecture, etc.
        pub properties: HashMap<String, String>,
    }

    /// Action result
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ActionResult {
        /// Output files
        pub output_files: Vec<OutputFile>,
        /// Output directories
        pub output_directories: Vec<OutputDirectory>,
        /// Exit code
        pub exit_code: i32,
        /// Stdout digest
        pub stdout_digest: Option<Digest>,
        /// Stderr digest
        pub stderr_digest: Option<Digest>,
        /// Execution metadata
        pub execution_metadata: ExecutionMetadata,
    }

    /// Output file
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OutputFile {
        pub path: String,
        pub digest: Digest,
        pub is_executable: bool,
    }

    /// Output directory
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OutputDirectory {
        pub path: String,
        pub tree_digest: Digest,
    }

    /// Execution metadata
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ExecutionMetadata {
        /// Worker ID that executed the action
        pub worker: String,
        /// When the action was queued
        pub queued_timestamp: std::time::SystemTime,
        /// When the worker started executing
        pub worker_start_timestamp: std::time::SystemTime,
        /// When the worker completed
        pub worker_completed_timestamp: std::time::SystemTime,
        /// When the inputs were fetched
        pub input_fetch_start_timestamp: std::time::SystemTime,
        /// When the inputs were ready
        pub input_fetch_completed_timestamp: std::time::SystemTime,
        /// When execution started
        pub execution_start_timestamp: std::time::SystemTime,
        /// When execution completed
        pub execution_completed_timestamp: std::time::SystemTime,
        /// When outputs were uploaded
        pub output_upload_start_timestamp: std::time::SystemTime,
        /// When outputs were ready
        pub output_upload_completed_timestamp: std::time::SystemTime,
    }

    /// Directory tree for CAS
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Directory {
        pub files: Vec<FileNode>,
        pub directories: Vec<DirectoryNode>,
        pub symlinks: Vec<SymlinkNode>,
    }

    /// File node in directory
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FileNode {
        pub name: String,
        pub digest: Digest,
        pub is_executable: bool,
    }

    /// Directory node
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DirectoryNode {
        pub name: String,
        pub digest: Digest,
    }

    /// Symlink node
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SymlinkNode {
        pub name: String,
        pub target: String,
    }
}

/// Statistics for the remote cache system
#[derive(Debug, Clone, Default)]
pub struct RemoteCacheStats {
    /// Action cache hits
    pub action_cache_hits: u64,
    /// Action cache misses
    pub action_cache_misses: u64,
    /// CAS hits
    pub cas_hits: u64,
    /// CAS misses
    pub cas_misses: u64,
    /// Total bytes uploaded
    pub bytes_uploaded: u64,
    /// Total bytes downloaded
    pub bytes_downloaded: u64,
    /// Actions executed
    pub actions_executed: u64,
    /// Actions cached
    pub actions_cached: u64,
    /// Current CAS size
    pub cas_size_bytes: u64,
    /// Number of objects in CAS
    pub cas_object_count: u64,
}

/// Error types for remote cache operations
#[derive(Debug, thiserror::Error)]
pub enum RemoteCacheError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("CAS error: {0}")]
    CAS(String),

    #[error("Sandbox error: {0}")]
    Sandbox(String),

    #[error("Digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("Action not found: {0}")]
    ActionNotFound(String),

    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RemoteCacheError>;
