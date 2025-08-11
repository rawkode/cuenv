//! Task server provider that exposes cuenv tasks to external tools (part 1)

use cuenv_config::Config;
use cuenv_core::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};

/// Task server provider that exposes cuenv tasks to external tools
pub struct TaskServerProvider {
    pub(crate) socket_path: Option<PathBuf>,
    pub(crate) listener: Option<UnixListener>,
    pub(crate) config: Arc<Config>,
    pub(crate) allow_exec: bool,
    pub(crate) use_stdio: bool,
}

impl TaskServerProvider {
    /// Create a new task server provider for Unix socket
    pub fn new(socket_path: PathBuf, config: Arc<Config>) -> Self {
        Self {
            socket_path: Some(socket_path),
            listener: None,
            config,
            allow_exec: false,
            use_stdio: false,
        }
    }

    /// Create a new task server provider for stdio (MCP mode)
    pub fn new_stdio(config: Arc<Config>, allow_exec: bool) -> Self {
        Self {
            socket_path: None,
            listener: None,
            config,
            allow_exec,
            use_stdio: true,
        }
    }

    /// Create a new task server provider with full options
    pub fn new_with_options(
        socket_path: Option<PathBuf>,
        config: Arc<Config>,
        allow_exec: bool,
        use_stdio: bool,
    ) -> Self {
        Self {
            socket_path,
            listener: None,
            config,
            allow_exec,
            use_stdio,
        }
    }

    /// Start the server and listen for connections
    pub async fn start(&mut self) -> Result<()> {
        if self.use_stdio {
            // Use stdio for MCP mode
            tracing::info!("Task server provider started in stdio mode for MCP");
            self.handle_stdio().await
        } else if let Some(socket_path) = &self.socket_path {
            // Remove socket if it exists
            if socket_path.exists() {
                tokio::fs::remove_file(socket_path).await.map_err(|e| {
                    Error::file_system(socket_path.clone(), "remove existing socket", e)
                })?;
            }

            // Ensure parent directory exists
            if let Some(parent) = socket_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::file_system(parent.to_path_buf(), "create socket parent directory", e)
                })?;
            }

            // Start Unix domain socket listener
            let listener = UnixListener::bind(socket_path).map_err(|e| {
                Error::configuration(format!(
                    "Failed to bind to socket {}: {}",
                    socket_path.display(),
                    e
                ))
            })?;

            self.listener = Some(listener);
            tracing::info!(
                socket_path = %socket_path.display(),
                "Task server provider started"
            );

            // Accept connections
            self.handle_connections().await
        } else {
            Err(Error::configuration(
                "No transport configured: need either socket_path or use_stdio".to_string(),
            ))
        }
    }

    /// Handle stdio communication for MCP mode
    async fn handle_stdio(&mut self) -> Result<()> {
        use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader};

        let stdin = stdin();
        let mut stdout = stdout();
        let mut buf_reader = BufReader::new(stdin);
        let mut line = String::new();

        while buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read from stdin: {e}")))?
            > 0
        {
            // Parse JSON-RPC request
            let request: serde_json::Value = serde_json::from_str(line.trim())
                .map_err(|e| Error::configuration(format!("Invalid JSON-RPC request: {e}")))?;

            // Handle the request
            let response =
                Self::handle_request(request, self.config.get_tasks(), self.allow_exec).await;

            // Send response
            let response_json = serde_json::to_string(&response)
                .map_err(|e| Error::configuration(format!("Failed to serialize response: {e}")))?;

            stdout
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .map_err(|e| Error::configuration(format!("Failed to write response: {e}")))?;

            stdout
                .flush()
                .await
                .map_err(|e| Error::configuration(format!("Failed to flush stdout: {e}")))?;

            line.clear();
        }

        Ok(())
    }

    /// Handle incoming client connections
    async fn handle_connections(&mut self) -> Result<()> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| Error::configuration("Task server provider not started".to_string()))?;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let config = Arc::clone(&self.config);
                    let allow_exec = self.allow_exec;
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, config, allow_exec).await {
                            tracing::error!(error = %e, "Client connection error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to accept connection");
                    // Continue accepting other connections
                }
            }
        }
    }

    /// Handle a single client connection
    async fn handle_client(
        stream: UnixStream,
        config: Arc<Config>,
        allow_exec: bool,
    ) -> Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let (read_half, mut write_half) = stream.into_split();
        let mut buf_reader = BufReader::new(read_half);
        let mut line = String::new();

        while buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| Error::configuration(format!("Failed to read from client: {e}")))?
            > 0
        {
            // Parse JSON-RPC request
            let request: serde_json::Value = serde_json::from_str(line.trim())
                .map_err(|e| Error::configuration(format!("Invalid JSON-RPC request: {e}")))?;

            // Handle the request
            let response = Self::handle_request(request, config.get_tasks(), allow_exec).await;

            // Send response
            let response_json = serde_json::to_string(&response)
                .map_err(|e| Error::configuration(format!("Failed to serialize response: {e}")))?;

            write_half
                .write_all(format!("{response_json}\n").as_bytes())
                .await
                .map_err(|e| Error::configuration(format!("Failed to write response: {e}")))?;

            line.clear();
        }

        Ok(())
    }
}
