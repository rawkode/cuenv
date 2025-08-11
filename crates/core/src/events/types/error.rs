//! Event system errors

/// Event system errors
#[derive(Debug, thiserror::Error)]
pub enum EventSystemError {
    #[error("Event system already initialized")]
    AlreadyInitialized,
    #[error("Event system not initialized")]
    NotInitialized,
    #[error("Subscriber error: {0}")]
    SubscriberError(String),
}
