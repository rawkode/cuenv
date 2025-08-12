use clap::Parser;
use cuenv_cache::CacheMode;
use cuenv_config::{ConfigLoader, RuntimeOptions};
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

    /// Environment to use (e.g., dev, staging, production)
    #[arg(short = 'e', long = "env", global = true)]
    environment: Option<String>,

    /// Capabilities to enable (can be specified multiple times)
    #[arg(short = 'c', long = "capability", global = true)]
    capabilities: Vec<String>,

    /// Run in audit mode to see file and network access without restrictions
    #[arg(long, global = true)]
    audit: bool,

    /// Output format for task execution (tui, spinner, simple, tree)
    #[arg(long, value_parser = ["tui", "spinner", "simple", "tree"])]
    output_format: Option<String>,

    /// Enable Chrome trace output
    #[arg(long)]
    trace_output: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Build runtime options from CLI arguments
    let runtime = RuntimeOptions {
        environment: cli.environment.clone(),
        capabilities: cli.capabilities.clone(),
        audit_mode: cli.audit,
        cache_mode: cli.cache.clone(),
        cache_enabled: cli.cache_enabled.unwrap_or(true),
        output_format: cli.output_format.clone(),
        trace_output: cli.trace_output,
    };

    // Set cache environment variables if provided
    if let Some(cache_mode) = cli.cache.clone() {
        let mode = match cache_mode.as_str() {
            "off" => CacheMode::Off,
            "read" => CacheMode::Read,
            "read-write" => CacheMode::ReadWrite,
            "write" => CacheMode::Write,
            _ => CacheMode::ReadWrite,
        };
        env::set_var("CUENV_CACHE_MODE", mode.to_string());
    }

    if let Some(enabled) = cli.cache_enabled {
        env::set_var("CUENV_CACHE_ENABLED", enabled.to_string());
    }

    // Determine the command to execute
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // Print help when no command is provided
            use clap::CommandFactory;
            Cli::command().print_help()?;
            return Ok(());
        }
    };

    // Load configuration once at startup
    let config = ConfigLoader::new()
        .runtime(runtime)
        .load()
        .await?
        .into_arc();

    // Execute the command with configuration
    command.execute(config).await.map_err(Into::into)
}
