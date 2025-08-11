//! EventSubscriber trait implementation for JSON logging

use super::config::JsonLogSubscriber;
use super::formatter::format_event;
use crate::events::{EnhancedEvent, EventSubscriber, SystemEvent};
use async_trait::async_trait;
use std::sync::atomic::Ordering;
use tracing::debug;

/// Wrapper type for EventSubscriber implementation
pub struct JsonLogEventSubscriber {
    inner: JsonLogSubscriber,
}

impl From<JsonLogSubscriber> for JsonLogEventSubscriber {
    fn from(inner: JsonLogSubscriber) -> Self {
        Self { inner }
    }
}

impl JsonLogEventSubscriber {
    /// Get the inner subscriber
    pub fn inner(&self) -> &JsonLogSubscriber {
        &self.inner
    }

    /// Flush pending writes
    pub async fn flush(&self) -> Result<(), super::error::JsonLogError> {
        self.inner.flush().await
    }
}

#[async_trait]
impl EventSubscriber for JsonLogSubscriber {
    async fn handle_event(
        &self,
        event: &EnhancedEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Format the event
        let formatted = format_event(event, self.config.include_metadata).await?;

        // Check for rotation before writing
        self.check_rotation().await?;

        // Write the log entry
        let bytes_written = self.writer.write(&formatted).await?;

        // Update cached file size estimate
        self.cached_file_size
            .fetch_add(bytes_written as u64, Ordering::Relaxed);

        debug!(
            event_type = std::any::type_name_of_val(&event.event),
            log_file = %self.config.file_path.display(),
            "JSON log event written"
        );

        Ok(())
    }

    fn name(&self) -> &'static str {
        "json_log"
    }

    fn is_interested(&self, _event: &SystemEvent) -> bool {
        // JSON logger is interested in all events
        true
    }
}

#[async_trait]
impl EventSubscriber for JsonLogEventSubscriber {
    async fn handle_event(
        &self,
        event: &EnhancedEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.handle_event(event).await
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn is_interested(&self, event: &SystemEvent) -> bool {
        self.inner.is_interested(event)
    }
}