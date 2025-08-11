//! Builder methods for creating errors with context

use super::types::Error;
use std::path::PathBuf;

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
