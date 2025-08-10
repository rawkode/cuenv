use std::path::PathBuf;

/// Result type alias for cuenv operations
pub type Result<T> = std::result::Result<T, Error>;

/// Core error type for cuenv operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// CUE file parsing errors
    #[error("failed to parse CUE file '{path}': {message}")]
    CueParse {
        path: PathBuf,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Environment variable related errors
    #[error("environment variable '{variable}' error: {message}")]
    Environment { variable: String, message: String },

    /// Secret resolution errors
    #[error("failed to resolve secret '{reference}': {message}")]
    SecretResolution {
        reference: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Command execution errors
    #[error("{}", format_command_error(.command, .args, .message, .exit_code))]
    CommandExecution {
        command: String,
        args: Vec<String>,
        message: String,
        exit_code: Option<i32>,
    },

    /// Configuration errors
    #[error("configuration error: {message}")]
    Configuration { message: String },

    /// Shell expansion errors
    #[error("failed to expand shell value '{value}': {message}")]
    ShellExpansion { value: String, message: String },

    /// File system operations
    #[error("file system {operation} operation failed for '{path}': {source}")]
    FileSystem {
        path: PathBuf,
        operation: String,
        #[source]
        source: std::io::Error,
    },

    /// JSON serialization/deserialization errors
    #[error("JSON error: {message}")]
    Json {
        message: String,
        #[source]
        source: serde_json::Error,
    },

    /// FFI errors from CUE operations
    #[error("FFI operation '{operation}' failed: {message}")]
    Ffi { operation: String, message: String },

    /// Permission denied errors
    #[error("permission denied for {operation}: {message}")]
    PermissionDenied { operation: String, message: String },

    /// Unsupported operation errors
    #[error("unsupported feature '{feature}': {message}")]
    Unsupported { feature: String, message: String },

    /// Security validation errors
    #[error("security validation error: {message}")]
    Security { message: String },

    /// Network-related errors
    #[error("network error for '{endpoint}': {message}")]
    Network { endpoint: String, message: String },

    /// Operation timeout errors
    #[error("operation '{operation}' timed out after {duration:?}")]
    Timeout {
        operation: String,
        duration: std::time::Duration,
    },
}

fn format_command_error(command: &str, args: &[String], message: &str, exit_code: &Option<i32>) -> String {
    let args_str = args.join(" ");
    match exit_code {
        Some(code) => {
            if args_str.is_empty() {
                format!("command '{command}' failed with exit code {code}: {message}")
            } else {
                format!("command '{command} {args_str}' failed with exit code {code}: {message}")
            }
        }
        None => {
            if args_str.is_empty() {
                format!("command '{command}' failed: {message}")
            } else {
                format!("command '{command} {args_str}' failed: {message}")
            }
        }
    }
}

// Conversion implementations
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::FileSystem {
            path: PathBuf::new(),
            operation: "unknown".to_string(),
            source: error,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json {
            message: error.to_string(),
            source: error,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Error::Configuration {
            message: format!("An internal error occurred: {error}"),
        }
    }
}

// Helper methods for creating errors with context
impl Error {
    /// Create a CUE parse error with context
    #[must_use]
    pub fn cue_parse(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::CueParse {
            path: path.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create a CUE parse error with a source error
    #[must_use]
    pub fn cue_parse_with_source(
        path: impl Into<PathBuf>,
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Error::CueParse {
            path: path.into(),
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Create an environment variable error
    #[must_use]
    pub fn environment(variable: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Environment {
            variable: variable.into(),
            message: message.into(),
        }
    }

    /// Create a secret resolution error
    #[must_use]
    pub fn secret_resolution(reference: impl Into<String>, message: impl Into<String>) -> Self {
        Error::SecretResolution {
            reference: reference.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create a command execution error
    #[must_use]
    pub fn command_execution(
        command: impl Into<String>,
        args: Vec<String>,
        message: impl Into<String>,
        exit_code: Option<i32>,
    ) -> Self {
        Error::CommandExecution {
            command: command.into(),
            args,
            message: message.into(),
            exit_code,
        }
    }

    /// Create a configuration error
    #[must_use]
    pub fn configuration(message: impl Into<String>) -> Self {
        Error::Configuration {
            message: message.into(),
        }
    }

    /// Create a shell expansion error
    #[must_use]
    pub fn shell_expansion(value: impl Into<String>, message: impl Into<String>) -> Self {
        Error::ShellExpansion {
            value: value.into(),
            message: message.into(),
        }
    }

    /// Create a file system error with context
    #[must_use]
    pub fn file_system(
        path: impl Into<PathBuf>,
        operation: impl Into<String>,
        source: std::io::Error,
    ) -> Self {
        Error::FileSystem {
            path: path.into(),
            operation: operation.into(),
            source,
        }
    }

    /// Create an FFI error
    #[must_use]
    pub fn ffi(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Ffi {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Create a permission denied error
    #[must_use]
    pub fn permission_denied(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Error::PermissionDenied {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Create an unsupported feature error
    #[must_use]
    pub fn unsupported(feature: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Unsupported {
            feature: feature.into(),
            message: message.into(),
        }
    }

    /// Create a security validation error
    #[must_use]
    pub fn security(message: impl Into<String>) -> Self {
        Error::Security {
            message: message.into(),
        }
    }

    /// Create a network error
    #[must_use]
    pub fn network(endpoint: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Network {
            endpoint: endpoint.into(),
            message: message.into(),
        }
    }

    /// Create a timeout error
    #[must_use]
    pub fn timeout(operation: impl Into<String>, duration: std::time::Duration) -> Self {
        Error::Timeout {
            operation: operation.into(),
            duration,
        }
    }
}

// Extension trait for adding context to Results
pub trait ResultExt<T> {
    /// Add context to a Result
    fn context(self, message: impl Into<String>) -> Result<T>;

    /// Add context with a lazy message
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Configuration {
                message: format!("{}: {}", message.into(), base_error),
            }
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Configuration {
                message: format!("{}: {}", f(), base_error),
            }
        })
    }
}
