//! Global event system management

use crate::events::emitter::EventEmitter;
use crate::events::subscriber::EventSubscriber;
use crate::events::types::{EventSystemError, SystemEvent};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Global event emitter instance
static GLOBAL_EVENT_EMITTER: std::sync::OnceLock<Arc<EventEmitter>> = std::sync::OnceLock::new();

/// Initialize global event system with custom capacity
pub fn initialize_global_events(capacity: usize) -> Result<Arc<EventEmitter>, EventSystemError> {
    let emitter = Arc::new(EventEmitter::new(capacity));

    GLOBAL_EVENT_EMITTER
        .set(emitter.clone())
        .map_err(|_| EventSystemError::AlreadyInitialized)?;

    debug!("Global event system initialized with capacity {}", capacity);
    Ok(emitter)
}

/// Get the global event emitter (initializing with default capacity if needed)
pub fn global_event_emitter() -> Arc<EventEmitter> {
    GLOBAL_EVENT_EMITTER
        .get_or_init(|| {
            debug!("Auto-initializing global event system with default capacity");
            Arc::new(EventEmitter::new(10000))
        })
        .clone()
}

/// Emit a global event
pub async fn emit_global_event(event: SystemEvent) {
    let emitter = global_event_emitter();
    emitter.emit(event).await;
}

/// Emit a global event with metadata
pub async fn emit_global_event_with_metadata(
    event: SystemEvent,
    metadata: HashMap<String, String>,
) {
    let emitter = global_event_emitter();
    emitter.emit_with_metadata(event, metadata).await;
}

/// Backward compatibility aliases
pub fn global_event_bus() -> Arc<EventEmitter> {
    global_event_emitter()
}

pub async fn publish_global_event(event: SystemEvent) {
    emit_global_event(event).await;
}

pub async fn register_global_subscriber(subscriber: Arc<dyn EventSubscriber>) {
    let emitter = global_event_emitter();
    emitter.add_subscriber(subscriber).await;
}
