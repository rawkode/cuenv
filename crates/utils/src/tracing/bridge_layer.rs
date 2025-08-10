//! Event Bridge Layer for Phase 3 architecture
//!
//! This layer bridges the existing tracing system with the new comprehensive event system.
//! It converts tracing events to application events without requiring changes to existing
//! tracing code.

use cuenv_core::events::{
    CacheEvent, PipelineEvent, SystemEvent, TaskEvent, global_event_emitter,
};
use std::collections::HashMap;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use uuid;

/// Event bridge layer that converts tracing events to application events
pub struct EventBridgeLayer {
    /// Whether to emit events to the global emitter
    enabled: bool,
}

/// Field visitor for extracting event data
struct EventFieldVisitor {
    fields: HashMap<String, String>,
}

impl EventBridgeLayer {
    /// Create a new event bridge layer
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled bridge layer
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Enable or disable the bridge layer
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Extract fields from a tracing event
    fn extract_event_fields(event: &Event<'_>) -> HashMap<String, String> {
        let mut visitor = EventFieldVisitor {
            fields: HashMap::new(),
        };
        event.record(&mut visitor);
        visitor.fields
    }

    /// Convert a tracing event to a system event
    fn convert_event(event: &Event<'_>, fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let target = event.metadata().target();
        let level = event.metadata().level();
        
        // Extract the message if it exists
        let message = fields.get("message").cloned().unwrap_or_default();

        // Match on event patterns based on target and message content
        match (target, message.as_str()) {
            // Task events
            (_, msg) if msg == "task_progress" => {
                Self::create_task_progress_event(fields)
            }
            (_, msg) if msg == "task_completed" => {
                Self::create_task_completed_event(fields)
            }
            (_, msg) if msg == "task_failed" => {
                Self::create_task_failed_event(fields)
            }
            (_, msg) if msg == "task_started" => {
                Self::create_task_started_event(fields)
            }
            (_, msg) if msg == "task_skipped" => {
                Self::create_task_skipped_event(fields)
            }

            // Cache events
            (_, msg) if msg == "cache_hit" => {
                Self::create_cache_hit_event(fields)
            }
            (_, msg) if msg == "cache_miss" => {
                Self::create_cache_miss_event(fields)
            }
            (_, msg) if msg == "cache_write" => {
                Self::create_cache_write_event(fields)
            }
            (_, msg) if msg == "cache_evict" => {
                Self::create_cache_evict_event(fields)
            }

            // Pipeline events
            (_, msg) if msg == "pipeline_started" => {
                Self::create_pipeline_started_event(fields)
            }
            (_, msg) if msg == "pipeline_completed" => {
                Self::create_pipeline_completed_event(fields)
            }
            (_, msg) if msg == "level_started" => {
                Self::create_level_started_event(fields)
            }
            (_, msg) if msg == "level_completed" => {
                Self::create_level_completed_event(fields)
            }

            // Ignore other events
            _ => None,
        }
    }

    fn create_task_progress_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let message = fields.get("message").cloned().unwrap_or_default();
        let task_id = fields.get("task_id").cloned().unwrap_or_else(|| {
            format!("{}-{}", task_name, uuid::Uuid::new_v4().simple())
        });

        Some(SystemEvent::Task(TaskEvent::TaskProgress {
            task_name,
            task_id,
            message,
        }))
    }

    fn create_task_completed_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let duration_ms = fields
            .get("duration_ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let task_id = fields.get("task_id").cloned().unwrap_or_else(|| {
            format!("{}-{}", task_name, uuid::Uuid::new_v4().simple())
        });

        Some(SystemEvent::Task(TaskEvent::TaskCompleted {
            task_name,
            task_id,
            duration_ms,
        }))
    }

    fn create_task_failed_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let error = fields.get("error").cloned().unwrap_or_else(|| "Unknown error".to_string());
        let task_id = fields.get("task_id").cloned().unwrap_or_else(|| {
            format!("{}-{}", task_name, uuid::Uuid::new_v4().simple())
        });

        Some(SystemEvent::Task(TaskEvent::TaskFailed {
            task_name,
            task_id,
            error,
        }))
    }

    fn create_task_started_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let task_id = fields.get("task_id").cloned().unwrap_or_else(|| {
            format!("{}-{}", task_name, uuid::Uuid::new_v4().simple())
        });

        Some(SystemEvent::Task(TaskEvent::TaskStarted {
            task_name,
            task_id,
        }))
    }

    fn create_task_skipped_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let reason = fields.get("reason").cloned().unwrap_or_else(|| "Cache hit".to_string());
        let task_id = fields.get("task_id").cloned().unwrap_or_else(|| {
            format!("{}-{}", task_name, uuid::Uuid::new_v4().simple())
        });

        Some(SystemEvent::Task(TaskEvent::TaskSkipped {
            task_name,
            task_id,
            reason,
        }))
    }

    fn create_cache_hit_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields.get("task_name").cloned().unwrap_or_else(|| "unknown".to_string())
        });

        Some(SystemEvent::Cache(CacheEvent::CacheHit { key }))
    }

    fn create_cache_miss_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields.get("task_name").cloned().unwrap_or_else(|| "unknown".to_string())
        });

        Some(SystemEvent::Cache(CacheEvent::CacheMiss { key }))
    }

    fn create_cache_write_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields.get("task_name").cloned().unwrap_or_else(|| "unknown".to_string())
        });
        let size_bytes = fields
            .get("size_bytes")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Some(SystemEvent::Cache(CacheEvent::CacheWrite { key, size_bytes }))
    }

    fn create_cache_evict_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields.get("task_name").cloned().unwrap_or_else(|| "unknown".to_string())
        });
        let reason = fields.get("reason").cloned().unwrap_or_else(|| "Unknown".to_string());

        Some(SystemEvent::Cache(CacheEvent::CacheEvict { key, reason }))
    }

    fn create_pipeline_started_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let total_tasks = fields
            .get("total_tasks")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let total_levels = fields
            .get("total_levels")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1);

        Some(SystemEvent::Pipeline(PipelineEvent::PipelineStarted {
            total_tasks,
            total_levels,
        }))
    }

    fn create_pipeline_completed_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let total_duration_ms = fields
            .get("total_duration_ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let successful_tasks = fields
            .get("successful_tasks")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let failed_tasks = fields
            .get("failed_tasks")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        Some(SystemEvent::Pipeline(PipelineEvent::PipelineCompleted {
            total_duration_ms,
            successful_tasks,
            failed_tasks,
        }))
    }

    fn create_level_started_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let level = fields
            .get("level")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let tasks_in_level = fields
            .get("tasks_in_level")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        Some(SystemEvent::Pipeline(PipelineEvent::LevelStarted {
            level,
            tasks_in_level,
        }))
    }

    fn create_level_completed_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let level = fields
            .get("level")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let successful_tasks = fields
            .get("successful_tasks")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let failed_tasks = fields
            .get("failed_tasks")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        Some(SystemEvent::Pipeline(PipelineEvent::LevelCompleted {
            level,
            successful_tasks,
            failed_tasks,
        }))
    }

    /// Emit the event to the global emitter asynchronously
    fn emit_event_async(event: SystemEvent) {
        // We need to spawn a task to handle the async emission
        let handle = tokio::spawn(async move {
            global_event_emitter().publish(event).await;
        });
        
        // Spawn a task to log if the spawned task panics
        tokio::spawn(async move {
            if let Err(e) = handle.await {
                tracing::error!("emit_event_async task panicked: {:?}", e);
            }
        });
    }
}

impl Default for EventBridgeLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for EventBridgeLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if !self.enabled {
            return;
        }

        // Extract fields from the event
        let fields = Self::extract_event_fields(event);

        // Convert tracing event to system event
        if let Some(system_event) = Self::convert_event(event, &fields) {
            // Emit the event asynchronously
            Self::emit_event_async(system_event);
        }
    }
}

impl Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(field.name().to_string(), format!("{:?}", value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{info, span, Level};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Registry;

    #[test]
    fn test_event_bridge_layer_creation() {
        let layer = EventBridgeLayer::new();
        assert!(layer.enabled);

        let disabled_layer = EventBridgeLayer::disabled();
        assert!(!disabled_layer.enabled);
    }

    #[test]
    fn test_field_extraction() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "test_task".to_string());
        fields.insert("duration_ms".to_string(), "1500".to_string());

        let event = EventBridgeLayer::create_task_completed_event(&fields);
        
        match event {
            Some(SystemEvent::Task(TaskEvent::TaskCompleted { task_name, duration_ms, .. })) => {
                assert_eq!(task_name, "test_task");
                assert_eq!(duration_ms, 1500);
            }
            _ => panic!("Expected TaskCompleted event"),
        }
    }

    #[test]
    fn test_cache_event_conversion() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "test-cache-key".to_string());

        let hit_event = EventBridgeLayer::create_cache_hit_event(&fields);
        match hit_event {
            Some(SystemEvent::Cache(CacheEvent::CacheHit { key })) => {
                assert_eq!(key, "test-cache-key");
            }
            _ => panic!("Expected CacheHit event"),
        }
    }

    #[test]
    fn test_pipeline_event_conversion() {
        let mut fields = HashMap::new();
        fields.insert("total_tasks".to_string(), "5".to_string());
        fields.insert("total_levels".to_string(), "3".to_string());

        let event = EventBridgeLayer::create_pipeline_started_event(&fields);
        match event {
            Some(SystemEvent::Pipeline(PipelineEvent::PipelineStarted { total_tasks, total_levels })) => {
                assert_eq!(total_tasks, 5);
                assert_eq!(total_levels, 3);
            }
            _ => panic!("Expected PipelineStarted event"),
        }
    }

    // Integration test would require a tokio runtime
    #[tokio::test]
    async fn test_bridge_layer_integration() {
        let bridge_layer = EventBridgeLayer::new();
        let subscriber = Registry::default().with(bridge_layer);

        tracing::subscriber::with_default(subscriber, || {
            // This would normally trigger event emission
            info!(
                task_name = "test_integration", 
                duration_ms = 2000,
                "task_completed"
            );
        });

        // In a real test, we'd verify the event was emitted to the global emitter
        // For now, this just verifies the layer doesn't panic
    }
}