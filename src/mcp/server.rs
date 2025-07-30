use crate::errors::{Error, Result};
use crate::mcp::tools::*;
use crate::mcp::types::McpServerOptions;
use rmcp::ServerHandler;

/// Run the MCP server with the given options
pub async fn run(options: McpServerOptions) -> Result<()> {
    // Create tool box with execution permissions
    let tool_box = CuenvToolBox {
        allow_exec: options.allow_exec,
    };

    // Configure transport based on options
    match options.transport.as_str() {
        "stdio" => {
            let mut server = ServerHandler::new_stdio(tool_box)
                .map_err(|e| Error::configuration(format!("Failed to create MCP server: {}", e)))?;
            
            server
                .run()
                .await
                .map_err(|e| Error::configuration(format!("MCP server error: {}", e)))?;
        }
        "tcp" => {
            let addr = format!("127.0.0.1:{}", options.port);
            let mut server = ServerHandler::new_tcp(tool_box, &addr)
                .await
                .map_err(|e| Error::configuration(format!("Failed to configure TCP transport: {}", e)))?;
            
            server
                .run()
                .await
                .map_err(|e| Error::configuration(format!("MCP server error: {}", e)))?;
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
