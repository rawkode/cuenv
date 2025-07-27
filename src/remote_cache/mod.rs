//! Remote cache server for Bazel/Buck2 compatibility
//!
//! This module implements a remote cache server that exposes cuenv's
//! cache infrastructure via the Bazel/Buck2 Remote Execution API protocol.

pub mod grpc_proto;
pub mod simple_server;

// Re-export main types
pub use simple_server::{RemoteCacheConfig, RemoteCacheServer};
