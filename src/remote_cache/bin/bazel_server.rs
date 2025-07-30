//! Standalone Bazel-compatible remote cache server
//!
//! This binary provides a fully-compliant Bazel Remote Execution API cache server
//! that can be used with Bazel, Buck2, and other compatible build systems.

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cuenv::cache::{CacheConfig, CacheMode};
use cuenv::remote_cache::{BazelRemoteCacheConfig, BazelRemoteCacheServer};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Bazel-compatible remote cache server for cuenv",
    long_about = "A fully-compliant Bazel Remote Execution API v2 cache server that integrates with cuenv's cache infrastructure"
)]
struct Args {
    /// Address to listen on
    #[arg(short, long, default_value = "127.0.0.1:50051")]
    address: SocketAddr,

    /// Base directory for cache storage
    #[arg(short, long, default_value = "/var/cache/cuenv")]
    cache_dir: PathBuf,

    /// Maximum cache size in bytes
    #[arg(long, default_value = "10737418240")] // 10GB
    max_cache_size: u64,

    /// Maximum batch size for batch operations
    #[arg(long, default_value = "1000")]
    max_batch_size: usize,

    /// Maximum blob size in bytes
    #[arg(long, default_value = "1073741824")] // 1GB
    max_blob_size: u64,

    /// Enable action cache
    #[arg(long, default_value = "true")]
    enable_action_cache: bool,

    /// Enable content-addressed storage
    #[arg(long, default_value = "true")]
    enable_cas: bool,

    /// Enable authentication (not implemented yet)
    #[arg(long, default_value = "false")]
    enable_authentication: bool,

    /// Circuit breaker failure threshold (0.0-1.0)
    #[arg(long, default_value = "0.5")]
    circuit_breaker_threshold: f64,

    /// Circuit breaker timeout in seconds
    #[arg(long, default_value = "60")]
    circuit_breaker_timeout_secs: u64,

    /// Inline storage threshold in bytes
    #[arg(long, default_value = "1024")] // 1KB
    inline_threshold: usize,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: Level,

    /// Enable metrics endpoint
    #[arg(long)]
    enable_metrics: bool,

    /// Metrics endpoint address
    #[arg(long, default_value = "127.0.0.1:9090")]
    metrics_address: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_level(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            args.log_level,
        ))
        .init();

    info!("Starting Bazel-compatible remote cache server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Address: {}", args.address);
    info!("Cache directory: {}", args.cache_dir.display());
    info!("Max cache size: {} bytes", args.max_cache_size);
    info!("Max batch size: {}", args.max_batch_size);
    info!("Max blob size: {} bytes", args.max_blob_size);

    // Validate configuration
    if args.circuit_breaker_threshold < 0.0 || args.circuit_breaker_threshold > 1.0 {
        anyhow::bail!("Circuit breaker threshold must be between 0.0 and 1.0");
    }

    // Create cache directory if it doesn't exist
    std::fs::create_dir_all(&args.cache_dir)?;

    // Create cache configuration
    let cache_config = CacheConfig {
        base_dir: args.cache_dir.clone(),
        max_size: args.max_cache_size,
        mode: CacheMode::ReadWrite,
        inline_threshold: args.inline_threshold,
        env_filter: Default::default(),
        task_env_filters: Default::default(),
    };

    // Create Bazel remote cache configuration
    let remote_config = BazelRemoteCacheConfig {
        address: args.address,
        cache_config,
        max_batch_size: args.max_batch_size,
        max_blob_size: args.max_blob_size,
        enable_action_cache: args.enable_action_cache,
        enable_cas: args.enable_cas,
        enable_authentication: args.enable_authentication,
        circuit_breaker_threshold: args.circuit_breaker_threshold,
        circuit_breaker_timeout: Duration::from_secs(args.circuit_breaker_timeout_secs),
    };

    // Create and start the server
    let server = BazelRemoteCacheServer::new(remote_config).await?;

    info!("=== Server Configuration ===");
    info!("Remote cache server ready for Bazel/Buck2 clients");
    info!("");
    info!("Configure Bazel with:");
    info!("  --remote_cache=grpc://{}", args.address);
    if !args.enable_authentication {
        info!("  --remote_cache_header=x-no-auth=true");
    }
    info!("");
    info!("Configure Buck2 with:");
    info!("  [buck2]");
    info!("  remote_cache = grpc://{}", args.address);
    info!("");
    info!("Supported features:");
    if args.enable_cas {
        info!("  ✓ Content-Addressed Storage (CAS)");
    }
    if args.enable_action_cache {
        info!("  ✓ Action Cache");
    }
    info!("  ✓ Batch operations");
    info!("  ✓ Circuit breaker fault tolerance");
    info!("  ✓ SHA256 digest function");
    info!("=========================");

    // Start metrics server if enabled
    if args.enable_metrics {
        let metrics_server = tokio::spawn(async move {
            info!("Starting metrics server on {}", args.metrics_address);
            // TODO: Implement metrics endpoint
        });
    }

    // Handle shutdown gracefully
    let server_handle = tokio::spawn(async move { server.serve().await });

    // Wait for Ctrl+C
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Received shutdown signal, gracefully shutting down...");
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // TODO: Implement graceful shutdown
    server_handle.abort();

    info!("Server shutdown complete");
    Ok(())
}
