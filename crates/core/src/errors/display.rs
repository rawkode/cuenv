//! Display implementations for error types

use super::types::Error;
use std::fmt;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CueParse { path, message, .. } => {
                write!(
                    f,
                    "failed to parse CUE file '{}': {}",
                    path.display(),
                    message
                )
            }
            Error::Environment { variable, message } => {
                write!(f, "environment variable '{variable}' error: {message}")
            }
            Error::SecretResolution {
                reference, message, ..
            } => {
                write!(f, "failed to resolve secret '{reference}': {message}")
            }
            Error::CommandExecution {
                command,
                args,
                message,
                exit_code,
            } => {
                let args_str = args.join(" ");
                match exit_code {
                    Some(code) => {
                        if args_str.is_empty() {
                            write!(
                                f,
                                "command '{command}' failed with exit code {code}: {message}"
                            )
                        } else {
                            write!(f, "command '{command} {args_str}' failed with exit code {code}: {message}")
                        }
                    }
                    None => {
                        if args_str.is_empty() {
                            write!(f, "command '{command}' failed: {message}")
                        } else {
                            write!(f, "command '{command} {args_str}' failed: {message}")
                        }
                    }
                }
            }
            Error::Configuration { message } => {
                write!(f, "configuration error: {message}")
            }
            Error::ShellExpansion { value, message } => {
                write!(f, "failed to expand shell value '{value}': {message}")
            }
            Error::FileSystem {
                path,
                operation,
                source,
            } => {
                write!(
                    f,
                    "file system {} operation failed for '{}': {}",
                    operation,
                    path.display(),
                    source
                )
            }
            Error::Json { message, .. } => {
                write!(f, "JSON error: {message}")
            }
            Error::Ffi { operation, message } => {
                write!(f, "FFI operation '{operation}' failed: {message}")
            }
            Error::PermissionDenied { operation, message } => {
                write!(f, "permission denied for {operation}: {message}")
            }
            Error::Unsupported { feature, message } => {
                write!(f, "unsupported feature '{feature}': {message}")
            }
            Error::Security { message } => {
                write!(f, "security validation error: {message}")
            }
            Error::Network { endpoint, message } => {
                write!(f, "network error for '{endpoint}': {message}")
            }
            Error::Timeout {
                operation,
                duration,
            } => {
                write!(f, "operation '{operation}' timed out after {duration:?}")
            }
        }
    }
}
