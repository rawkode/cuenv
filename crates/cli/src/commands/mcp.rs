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
            println!("Starting cuenv MCP server (stdio transport)");
            println!("Transport: stdio");
            println!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            println!("Ready for MCP clients (like Claude Code)");

            TaskServerProvider::new_stdio(Arc::clone(&config), allow_exec)
        }
        "unix" => {
            let socket_path = socket.unwrap_or_else(|| {
                tempfile::tempdir()
                    .map(|d| d.path().join("cuenv-mcp.sock"))
                    .unwrap_or_else(|_| PathBuf::from("/tmp/cuenv-mcp.sock"))
            });

            println!("Starting cuenv MCP server (Unix socket transport)");
            println!("Socket: {}", socket_path.display());
            println!(
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
            println!("Starting cuenv MCP server (TCP transport)");
            println!("Port: {}", port);
            println!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            println!("Note: TCP transport uses Unix socket internally - external TCP not implemented yet");

            // For TCP, we'll create a temporary socket and note the limitation
            let temp_socket = tempfile::tempdir()
                .map(|d| d.path().join("cuenv-mcp-tcp.sock"))
                .map_err(|e| {
                    Error::configuration(format!("Failed to create temp socket: {}", e))
                })?;

            TaskServerProvider::new_with_options(
                Some(temp_socket),
                Arc::clone(&config),
                allow_exec,
                false,
            )
        }
        _ => {
            return Err(Error::configuration(format!(
                "Unsupported transport: {}. Use 'stdio', 'unix', or 'tcp'",
                transport
            )));
        }
    };

    // Start the server (this will block until interrupted)
    println!("Press Ctrl+C to stop the server");

    // Set up signal handling for graceful shutdown
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        result = provider.start() => {
            match result {
                Ok(()) => println!("MCP server stopped successfully"),
                Err(e) => {
                    eprintln!("MCP server error: {}", e);
                    return Err(e);
                }
            }
        }
        _ = ctrl_c => {
            println!("Received interrupt signal, stopping MCP server...");
            if let Err(e) = provider.shutdown().await {
                eprintln!("Error during shutdown: {}", e);
            }
        }
    }

    Ok(())
}
