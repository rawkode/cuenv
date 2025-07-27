//! Standalone remote cache server for Bazel/Buck2 integration
use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cuenv::cache::{CacheConfig, CacheMode};
use cuenv::remote_cache::{RemoteCacheConfig, RemoteCacheServer};

#[derive(Parser, Debug)]
#[command(author, version, about = "cuenv remote cache server for Bazel/Buck2")]
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

    /// Enable action cache
    #[arg(long, default_value = "true")]
    enable_action_cache: bool,

    /// Enable content-addressed storage
    #[arg(long, default_value = "true")]
    enable_cas: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: Level,
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
                .with_thread_ids(true),
        )
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            args.log_level,
        ))
        .init();

    info!("Starting cuenv remote cache server on {}", args.address);
    info!("Cache directory: {}", args.cache_dir.display());
    info!("Max cache size: {} bytes", args.max_cache_size);

    // Create cache configuration
    let cache_config = CacheConfig {
        base_dir: args.cache_dir.clone(),
        max_size: args.max_cache_size,
        mode: CacheMode::ReadWrite,
        inline_threshold: 1024, // 1KB for inline storage
    };

    // Create remote cache configuration
    let remote_config = RemoteCacheConfig {
        address: args.address,
        enable_action_cache: args.enable_action_cache,
        enable_cas: args.enable_cas,
        cache_config,
    };

    // Create and start the server
    let server = RemoteCacheServer::new(remote_config).await?;
    
    info!("Remote cache server ready for Bazel/Buck2 clients");
    info!("Configure Bazel with:");
    info!("  --remote_cache=grpc://{}", args.address);
    
    server.serve().await?;

    Ok(())
}