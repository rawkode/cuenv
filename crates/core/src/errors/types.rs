//! Core error type definitions

use std::path::PathBuf;

/// Result type alias for cuenv operations
pub type Result<T> = std::result::Result<T, Error>;

/// Core error type for cuenv operations using thiserror
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// CUE file parsing errors
    CueParse {
        path: PathBuf,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Environment variable related errors
    Environment { variable: String, message: String },

    /// Secret resolution errors
    SecretResolution {
        reference: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Command execution errors
    CommandExecution {
        command: String,
        args: Vec<String>,
        message: String,
        exit_code: Option<i32>,
    },

    /// Configuration errors
    Configuration { message: String },

    /// Shell expansion errors
    ShellExpansion { value: String, message: String },

    /// File system operations
    FileSystem {
        path: PathBuf,
        operation: String,
        #[source]
        source: std::io::Error,
    },

    /// JSON serialization/deserialization errors
    Json {
        message: String,
        #[source]
        source: serde_json::Error,
    },

    /// FFI errors from CUE operations
    Ffi { operation: String, message: String },

    /// Permission denied errors
    PermissionDenied { operation: String, message: String },

    /// Unsupported operation errors
    Unsupported { feature: String, message: String },

    /// Security validation errors
    Security { message: String },

    /// Network-related errors
    Network { endpoint: String, message: String },

    /// Operation timeout errors
    Timeout {
        operation: String,
        duration: std::time::Duration,
    },
}
