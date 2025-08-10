use clap::Parser;
use cuenv_cache::CacheMode;
use std::env;

mod commands;
mod completion;
mod directory;
mod execute;
mod monorepo;
mod platform;

use commands::Commands;

#[derive(Parser)]
#[command(name = "cuenv")]
#[command(about = "A direnv alternative using CUE files", long_about = None)]
#[command(version)]
struct Cli {
    /// Cache mode (off, read, read-write, write)
    #[arg(long, value_parser = ["off", "read", "read-write", "write"])]
    cache: Option<String>,

    /// Enable or disable caching globally
    #[arg(long)]
    cache_enabled: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Set cache mode if provided
    if let Some(cache_mode) = cli.cache {
        let mode = match cache_mode.as_str() {
            "off" => CacheMode::Off,
            "read" => CacheMode::Read,
            "read-write" => CacheMode::ReadWrite,
            "write" => CacheMode::Write,
            _ => CacheMode::ReadWrite,
        };
        env::set_var("CUENV_CACHE_MODE", mode.to_string());
    }

    // Handle cache enabled flag
    if let Some(enabled) = cli.cache_enabled {
        env::set_var("CUENV_CACHE_ENABLED", enabled.to_string());
    }

    // Execute the command
    if let Some(command) = cli.command {
        execute::execute_command(command).await
    } else {
        // Default behavior when no command is specified
        Commands::Reload.execute().await
    }
}