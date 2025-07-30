use crate::errors::{Error, Result};
use crate::mcp::tools::*;
use crate::mcp::types::McpServerOptions;
use rmcp::{model::ServerCapabilities, model::ServerInfo, ServiceExt};

// Implement ServerHandler trait for CuenvToolBox
impl rmcp::ServerHandler for CuenvToolBox {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "cuenv MCP server provides environment management and task execution capabilities.\n\
                 Use --allow-exec flag to enable task execution."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

/// Run the MCP server with the given options
pub async fn run(options: McpServerOptions) -> Result<()> {
    // Create tool box with execution permissions
    let tool_box = CuenvToolBox {
        allow_exec: options.allow_exec,
    };

    // Configure transport based on options
    match options.transport.as_str() {
        "stdio" => {
            use rmcp::transport::stdio;

            // Create stdio transport
            let transport = stdio();

            // Start the server
            let server = tool_box
                .serve(transport)
                .await
                .map_err(|e| Error::configuration(format!("Failed to start MCP server: {e}")))?;

            // Wait for server to complete
            let _shutdown_reason = server
                .waiting()
                .await
                .map_err(|e| Error::configuration(format!("MCP server error: {e}")))?;
        }
        "tcp" => {
            use tokio::net::TcpListener;

            let addr = format!("127.0.0.1:{}", options.port);
            let listener = TcpListener::bind(&addr)
                .await
                .map_err(|e| Error::configuration(format!("Failed to bind to {addr}: {e}")))?;

            println!("MCP server listening on {addr}");

            // Accept connections in a loop
            loop {
                let (stream, addr) = listener.accept().await.map_err(|e| {
                    Error::configuration(format!("Failed to accept connection: {e}"))
                })?;

                println!("New connection from: {addr}");

                let tool_box_clone = tool_box.clone();

                // Spawn a task to handle each connection
                tokio::spawn(async move {
                    let server = match tool_box_clone.serve(stream).await {
                        Ok(server) => server,
                        Err(e) => {
                            eprintln!("Failed to serve connection from {addr}: {e}");
                            return;
                        }
                    };

                    match server.waiting().await {
                        Ok(reason) => {
                            println!("Connection from {addr} closed: {reason:?}");
                        }
                        Err(e) => {
                            eprintln!("Error serving connection from {addr}: {e}");
                        }
                    }
                });
            }
        }
        _ => {
            return Err(Error::configuration(format!(
                "Unsupported transport: {}. Use 'stdio' or 'tcp'",
                options.transport
            )));
        }
    }

    Ok(())
}
