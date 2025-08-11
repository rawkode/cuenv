//! Error types for JSON log operations

/// JSON log subscriber errors
#[derive(Debug, thiserror::Error)]
pub enum JsonLogError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
