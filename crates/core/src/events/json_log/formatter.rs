//! JSON formatting for events

use super::error::JsonLogError;
use crate::events::EnhancedEvent;

/// Format an event as JSON
pub async fn format_event(
    event: &EnhancedEvent,
    include_metadata: bool,
) -> Result<String, JsonLogError> {
    let mut json_obj = serde_json::json!({
        "timestamp": event.timestamp.duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| JsonLogError::SerializationError(e.to_string()))?
            .as_millis(),
        "event": event.event,
    });

    if include_metadata {
        if let Some(correlation_id) = &event.correlation_id {
            json_obj["correlation_id"] = serde_json::Value::String(correlation_id.clone());
        }

        if !event.metadata.is_empty() {
            json_obj["metadata"] = serde_json::Value::Object(
                event
                    .metadata
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            );
        }
    }

    serde_json::to_string(&json_obj).map_err(|e| JsonLogError::SerializationError(e.to_string()))
}