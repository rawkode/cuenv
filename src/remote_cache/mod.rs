//! Remote cache client for Bazel/Buck2 compatibility
//!
//! This module implements a remote cache client that can connect to
//! any Bazel/Buck2 Remote Execution API compatible cache server.

pub mod bazel_server;
pub mod client;
pub mod grpc_proto;

// Re-export main types
pub use bazel_server::{BazelRemoteCacheConfig, BazelRemoteCacheServer};
pub use client::{RemoteCacheClient, RemoteCacheClientBuilder, RemoteCacheClientConfig};
