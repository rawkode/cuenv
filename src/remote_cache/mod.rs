//! Remote cache client for Bazel/Buck2 compatibility
//!
//! This module implements a remote cache client that can connect to
//! any Bazel/Buck2 Remote Execution API compatible cache server.

pub mod client;

// Re-export main types
pub use client::{RemoteCacheClient, RemoteCacheClientBuilder, RemoteCacheClientConfig};
