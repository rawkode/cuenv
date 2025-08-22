//! Log message events

use serde::{Deserialize, Serialize};

/// Log level for messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Log message events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEvent {
    /// A log message
    Message {
        level: LogLevel,
        message: String,
        target: Option<String>,
    },
    /// Terminal capability fallback warning
    CapabilityFallback {
        requested_format: String,
        actual_format: String,
        reason: String,
    },
}
