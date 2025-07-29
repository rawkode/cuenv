use crate::errors::{Error, Result};
use crate::mcp::tools::*;
use crate::mcp::types::McpServerOptions;
use rmcp::ServerBuilder;
use std::io;

/// Run the MCP server with the given options
pub async fn run(options: McpServerOptions) -> Result<()> {
    let mut builder = ServerBuilder::new().tool_box(CuenvToolBox {
        allow_exec: options.allow_exec,
    });

    // Configure transport based on options
    let server = match options.transport.as_str() {
        "stdio" => {
            builder = builder.stdio();
            builder
                .build()
                .map_err(|e| Error::configuration(format!("Failed to create MCP server: {}", e)))?
        }
        "tcp" => {
            let addr = format!("127.0.0.1:{}", options.port);
            builder = builder.tcp(&addr).map_err(|e| {
                Error::configuration(format!("Failed to configure TCP transport: {}", e))
            })?;
            builder
                .build()
                .map_err(|e| Error::configuration(format!("Failed to create MCP server: {}", e)))?
        }
        _ => {
            return Err(Error::configuration(format!(
                "Unsupported transport: {}. Use 'stdio' or 'tcp'",
                options.transport
            )));
        }
    };

    // Run the server
    server
        .run()
        .await
        .map_err(|e| Error::configuration(format!("MCP server error: {}", e)))?;

    Ok(())
}
