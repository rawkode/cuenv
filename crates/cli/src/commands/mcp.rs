use cuenv_config::Config;
use cuenv_core::{Error, Result};
use cuenv_task::TaskServerProvider;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn execute(
    config: Arc<Config>,
    transport: String,
    port: u16,
    socket: Option<PathBuf>,
    allow_exec: bool,
) -> Result<()> {
    // Create the appropriate server based on transport
    let mut provider = match transport.as_str() {
        "stdio" => {
            tracing::info!("Starting cuenv MCP server (stdio transport)");
            tracing::info!("Transport: stdio");
            tracing::info!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            tracing::info!("Ready for MCP clients (like Claude Code)");

            TaskServerProvider::new_stdio(Arc::clone(&config), allow_exec)
        }
        "unix" => {
            let socket_path = socket.unwrap_or_else(|| {
                tempfile::tempdir()
                    .map(|d| d.path().join("cuenv-mcp.sock"))
                    .unwrap_or_else(|_| PathBuf::from("/tmp/cuenv-mcp.sock"))
            });

            tracing::info!("Starting cuenv MCP server (Unix socket transport)");
            tracing::info!("Socket: {}", socket_path.display());
            tracing::info!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );

            TaskServerProvider::new_with_options(
                Some(socket_path),
                Arc::clone(&config),
                allow_exec,
                false,
            )
        }
        "tcp" => {
            tracing::info!("Starting cuenv MCP server (TCP transport)");
            tracing::info!("Port: {port}");
            tracing::info!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            tracing::info!("Note: TCP transport uses Unix socket internally - external TCP not implemented yet");

            // For TCP, we'll create a temporary socket and note the limitation
            let temp_socket = tempfile::tempdir()
                .map(|d| d.path().join("cuenv-mcp-tcp.sock"))
                .map_err(|e| Error::configuration(format!("Failed to create temp socket: {e}")))?;

            TaskServerProvider::new_with_options(
                Some(temp_socket),
                Arc::clone(&config),
                allow_exec,
                false,
            )
        }
        _ => {
            return Err(Error::configuration(format!(
                "Unsupported transport: {transport}. Use 'stdio', 'unix', or 'tcp'"
            )));
        }
    };

    // Start the server (this will block until interrupted)
    tracing::info!("Press Ctrl+C to stop the server");

    // Set up signal handling for graceful shutdown
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        result = provider.start() => {
            match result {
                Ok(()) => tracing::info!("MCP server stopped successfully"),
                Err(e) => {
                    tracing::error!("MCP server error: {e}");
                    return Err(e);
                }
            }
        }
        _ = ctrl_c => {
            tracing::info!("Received interrupt signal, stopping MCP server...");
            if let Err(e) = provider.shutdown().await {
                tracing::error!("Error during shutdown: {e}");
            }
        }
    }

    Ok(())
}
