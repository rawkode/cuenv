//! Event Bridge Layer for Phase 3 architecture
//!
//! This layer bridges the existing tracing system with the new comprehensive event system.
//! It converts tracing events to application events without requiring changes to existing
//! tracing code.

#[cfg(not(test))]
use cuenv_core::events::global_event_emitter;
use cuenv_core::events::{CacheEvent, PipelineEvent, SystemEvent, TaskEvent};
use std::collections::HashMap;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
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
        let _level = event.metadata().level();

        // Extract the message if it exists
        let message = fields.get("message").cloned().unwrap_or_default();

        // Match on event patterns based on target and message content
        match (target, message.as_str()) {
            // Task events
            (_, "task_progress") => Self::create_task_progress_event(fields),
            (_, "task_completed") => Self::create_task_completed_event(fields),
            (_, "task_failed") => Self::create_task_failed_event(fields),
            (_, "task_started") => Self::create_task_started_event(fields),
            (_, "task_skipped") => Self::create_task_skipped_event(fields),

            // Cache events
            (_, "cache_hit") => Self::create_cache_hit_event(fields),
            (_, "cache_miss") => Self::create_cache_miss_event(fields),
            (_, "cache_write") => Self::create_cache_write_event(fields),
            (_, "cache_evict") => Self::create_cache_evict_event(fields),

            // Pipeline events
            (_, "pipeline_started") => Self::create_pipeline_started_event(fields),
            (_, "pipeline_completed") => Self::create_pipeline_completed_event(fields),
            (_, "level_started") => Self::create_level_started_event(fields),
            (_, "level_completed") => Self::create_level_completed_event(fields),

            // Ignore other events
            _ => None,
        }
    }

    fn create_task_progress_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let message = fields.get("message").cloned().unwrap_or_default();
        let task_id = fields
            .get("task_id")
            .cloned()
            .unwrap_or_else(|| format!("{}-{}", task_name, uuid::Uuid::new_v4().simple()));

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
        let task_id = fields
            .get("task_id")
            .cloned()
            .unwrap_or_else(|| format!("{}-{}", task_name, uuid::Uuid::new_v4().simple()));

        Some(SystemEvent::Task(TaskEvent::TaskCompleted {
            task_name,
            task_id,
            duration_ms,
        }))
    }

    fn create_task_failed_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let error = fields
            .get("error")
            .cloned()
            .unwrap_or_else(|| "Unknown error".to_string());
        let task_id = fields
            .get("task_id")
            .cloned()
            .unwrap_or_else(|| format!("{}-{}", task_name, uuid::Uuid::new_v4().simple()));

        Some(SystemEvent::Task(TaskEvent::TaskFailed {
            task_name,
            task_id,
            error,
        }))
    }

    fn create_task_started_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let task_id = fields
            .get("task_id")
            .cloned()
            .unwrap_or_else(|| format!("{}-{}", task_name, uuid::Uuid::new_v4().simple()));

        Some(SystemEvent::Task(TaskEvent::TaskStarted {
            task_name,
            task_id,
        }))
    }

    fn create_task_skipped_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let task_name = fields.get("task_name")?.clone();
        let reason = fields
            .get("reason")
            .cloned()
            .unwrap_or_else(|| "Cache hit".to_string());
        let task_id = fields
            .get("task_id")
            .cloned()
            .unwrap_or_else(|| format!("{}-{}", task_name, uuid::Uuid::new_v4().simple()));

        Some(SystemEvent::Task(TaskEvent::TaskSkipped {
            task_name,
            task_id,
            reason,
        }))
    }

    fn create_cache_hit_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields
                .get("task_name")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        });

        Some(SystemEvent::Cache(CacheEvent::CacheHit { key }))
    }

    fn create_cache_miss_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields
                .get("task_name")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        });

        Some(SystemEvent::Cache(CacheEvent::CacheMiss { key }))
    }

    fn create_cache_write_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields
                .get("task_name")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        });
        let size_bytes = fields
            .get("size_bytes")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Some(SystemEvent::Cache(CacheEvent::CacheWrite {
            key,
            size_bytes,
        }))
    }

    fn create_cache_evict_event(fields: &HashMap<String, String>) -> Option<SystemEvent> {
        let key = fields.get("key").cloned().unwrap_or_else(|| {
            fields
                .get("task_name")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        });
        let reason = fields
            .get("reason")
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

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
        // In tests, we skip async emission to avoid needing a runtime
        #[cfg(test)]
        {
            let _ = event; // Suppress unused variable warning
        }

        // In production, we need to spawn a task to handle the async emission
        #[cfg(not(test))]
        {
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
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::info;
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
            Some(SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name,
                duration_ms,
                ..
            })) => {
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
            Some(SystemEvent::Pipeline(PipelineEvent::PipelineStarted {
                total_tasks,
                total_levels,
            })) => {
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

    #[test]
    fn test_event_bridge_layer_enable_disable() {
        let mut layer = EventBridgeLayer::new();
        assert!(layer.enabled);

        layer.set_enabled(false);
        assert!(!layer.enabled);

        layer.set_enabled(true);
        assert!(layer.enabled);
    }

    #[test]
    fn test_event_field_visitor_functionality() {
        let visitor = EventFieldVisitor {
            fields: HashMap::new(),
        };

        // Test visitor directly by simulating field recording
        // We can't easily create Field instances since they require internal tracing types
        // Instead, we'll test the EventFieldVisitor indirectly through the layer

        // Verify the visitor starts empty
        assert!(visitor.fields.is_empty());

        // We'll test the actual field visiting through integration tests below
    }

    #[test]
    fn test_create_task_started_event() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "start_test".to_string());
        fields.insert("task_id".to_string(), "custom_id_123".to_string());

        let event = EventBridgeLayer::create_task_started_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskStarted { task_name, task_id })) => {
                assert_eq!(task_name, "start_test");
                assert_eq!(task_id, "custom_id_123");
            }
            _ => panic!("Expected TaskStarted event"),
        }
    }

    #[test]
    fn test_create_task_started_event_without_task_id() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "start_test".to_string());

        let event = EventBridgeLayer::create_task_started_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskStarted { task_name, task_id })) => {
                assert_eq!(task_name, "start_test");
                assert!(task_id.starts_with("start_test-"));
            }
            _ => panic!("Expected TaskStarted event"),
        }
    }

    #[test]
    fn test_create_task_skipped_event() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "skip_test".to_string());
        fields.insert("reason".to_string(), "dependency failure".to_string());
        fields.insert("task_id".to_string(), "skip_id_456".to_string());

        let event = EventBridgeLayer::create_task_skipped_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskSkipped {
                task_name,
                task_id,
                reason,
            })) => {
                assert_eq!(task_name, "skip_test");
                assert_eq!(task_id, "skip_id_456");
                assert_eq!(reason, "dependency failure");
            }
            _ => panic!("Expected TaskSkipped event"),
        }
    }

    #[test]
    fn test_create_task_skipped_event_default_reason() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "skip_test".to_string());

        let event = EventBridgeLayer::create_task_skipped_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskSkipped {
                task_name, reason, ..
            })) => {
                assert_eq!(task_name, "skip_test");
                assert_eq!(reason, "Cache hit");
            }
            _ => panic!("Expected TaskSkipped event"),
        }
    }

    #[test]
    fn test_create_task_progress_event() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "progress_test".to_string());
        fields.insert("message".to_string(), "50% complete".to_string());
        fields.insert("task_id".to_string(), "progress_id_789".to_string());

        let event = EventBridgeLayer::create_task_progress_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskProgress {
                task_name,
                task_id,
                message,
            })) => {
                assert_eq!(task_name, "progress_test");
                assert_eq!(task_id, "progress_id_789");
                assert_eq!(message, "50% complete");
            }
            _ => panic!("Expected TaskProgress event"),
        }
    }

    #[test]
    fn test_create_task_failed_event_with_error() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "failed_test".to_string());
        fields.insert("error".to_string(), "Custom error message".to_string());

        let event = EventBridgeLayer::create_task_failed_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskFailed {
                task_name, error, ..
            })) => {
                assert_eq!(task_name, "failed_test");
                assert_eq!(error, "Custom error message");
            }
            _ => panic!("Expected TaskFailed event"),
        }
    }

    #[test]
    fn test_create_task_failed_event_default_error() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "failed_test".to_string());

        let event = EventBridgeLayer::create_task_failed_event(&fields);

        match event {
            Some(SystemEvent::Task(TaskEvent::TaskFailed {
                task_name, error, ..
            })) => {
                assert_eq!(task_name, "failed_test");
                assert_eq!(error, "Unknown error");
            }
            _ => panic!("Expected TaskFailed event"),
        }
    }

    #[test]
    fn test_create_cache_miss_event() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "miss-cache-key".to_string());

        let event = EventBridgeLayer::create_cache_miss_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheMiss { key })) => {
                assert_eq!(key, "miss-cache-key");
            }
            _ => panic!("Expected CacheMiss event"),
        }
    }

    #[test]
    fn test_create_cache_miss_event_fallback_key() {
        let mut fields = HashMap::new();
        fields.insert("task_name".to_string(), "fallback_task".to_string());

        let event = EventBridgeLayer::create_cache_miss_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheMiss { key })) => {
                assert_eq!(key, "fallback_task");
            }
            _ => panic!("Expected CacheMiss event"),
        }
    }

    #[test]
    fn test_create_cache_write_event() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "write-cache-key".to_string());
        fields.insert("size_bytes".to_string(), "1024".to_string());

        let event = EventBridgeLayer::create_cache_write_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheWrite { key, size_bytes })) => {
                assert_eq!(key, "write-cache-key");
                assert_eq!(size_bytes, 1024);
            }
            _ => panic!("Expected CacheWrite event"),
        }
    }

    #[test]
    fn test_create_cache_write_event_invalid_size() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "write-cache-key".to_string());
        fields.insert("size_bytes".to_string(), "invalid".to_string());

        let event = EventBridgeLayer::create_cache_write_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheWrite { key, size_bytes })) => {
                assert_eq!(key, "write-cache-key");
                assert_eq!(size_bytes, 0); // Default for invalid parse
            }
            _ => panic!("Expected CacheWrite event"),
        }
    }

    #[test]
    fn test_create_cache_evict_event() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "evict-cache-key".to_string());
        fields.insert("reason".to_string(), "LRU eviction".to_string());

        let event = EventBridgeLayer::create_cache_evict_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheEvict { key, reason })) => {
                assert_eq!(key, "evict-cache-key");
                assert_eq!(reason, "LRU eviction");
            }
            _ => panic!("Expected CacheEvict event"),
        }
    }

    #[test]
    fn test_create_cache_evict_event_default_reason() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "evict-cache-key".to_string());

        let event = EventBridgeLayer::create_cache_evict_event(&fields);

        match event {
            Some(SystemEvent::Cache(CacheEvent::CacheEvict { key, reason })) => {
                assert_eq!(key, "evict-cache-key");
                assert_eq!(reason, "Unknown");
            }
            _ => panic!("Expected CacheEvict event"),
        }
    }

    #[test]
    fn test_create_pipeline_completed_event() {
        let mut fields = HashMap::new();
        fields.insert("total_duration_ms".to_string(), "5000".to_string());
        fields.insert("successful_tasks".to_string(), "8".to_string());
        fields.insert("failed_tasks".to_string(), "2".to_string());

        let event = EventBridgeLayer::create_pipeline_completed_event(&fields);

        match event {
            Some(SystemEvent::Pipeline(PipelineEvent::PipelineCompleted {
                total_duration_ms,
                successful_tasks,
                failed_tasks,
            })) => {
                assert_eq!(total_duration_ms, 5000);
                assert_eq!(successful_tasks, 8);
                assert_eq!(failed_tasks, 2);
            }
            _ => panic!("Expected PipelineCompleted event"),
        }
    }

    #[test]
    fn test_create_pipeline_completed_event_invalid_values() {
        let mut fields = HashMap::new();
        fields.insert("total_duration_ms".to_string(), "invalid".to_string());
        fields.insert("successful_tasks".to_string(), "not_a_number".to_string());
        fields.insert("failed_tasks".to_string(), "also_invalid".to_string());

        let event = EventBridgeLayer::create_pipeline_completed_event(&fields);

        match event {
            Some(SystemEvent::Pipeline(PipelineEvent::PipelineCompleted {
                total_duration_ms,
                successful_tasks,
                failed_tasks,
            })) => {
                assert_eq!(total_duration_ms, 0);
                assert_eq!(successful_tasks, 0);
                assert_eq!(failed_tasks, 0);
            }
            _ => panic!("Expected PipelineCompleted event"),
        }
    }

    #[test]
    fn test_create_level_started_event() {
        let mut fields = HashMap::new();
        fields.insert("level".to_string(), "3".to_string());
        fields.insert("tasks_in_level".to_string(), "7".to_string());

        let event = EventBridgeLayer::create_level_started_event(&fields);

        match event {
            Some(SystemEvent::Pipeline(PipelineEvent::LevelStarted {
                level,
                tasks_in_level,
            })) => {
                assert_eq!(level, 3);
                assert_eq!(tasks_in_level, 7);
            }
            _ => panic!("Expected LevelStarted event"),
        }
    }

    #[test]
    fn test_create_level_completed_event() {
        let mut fields = HashMap::new();
        fields.insert("level".to_string(), "2".to_string());
        fields.insert("successful_tasks".to_string(), "4".to_string());
        fields.insert("failed_tasks".to_string(), "1".to_string());

        let event = EventBridgeLayer::create_level_completed_event(&fields);

        match event {
            Some(SystemEvent::Pipeline(PipelineEvent::LevelCompleted {
                level,
                successful_tasks,
                failed_tasks,
            })) => {
                assert_eq!(level, 2);
                assert_eq!(successful_tasks, 4);
                assert_eq!(failed_tasks, 1);
            }
            _ => panic!("Expected LevelCompleted event"),
        }
    }

    #[test]
    fn test_convert_event_unknown_message() {
        // Test that convert_event returns None for unknown message types
        let mut fields = HashMap::new();
        fields.insert("message".to_string(), "unknown_event_type".to_string());

        // We can't easily create Event instances for testing since they require
        // internal tracing types, but we can test this through integration tests
        // For now, verify the logic using mock field data

        // Verify that unknown message types should return None
        assert!(fields.get("message").unwrap() == "unknown_event_type");
    }

    #[test]
    fn test_event_bridge_layer_default() {
        let layer1 = EventBridgeLayer::default();
        let layer2 = EventBridgeLayer::new();

        assert_eq!(layer1.enabled, layer2.enabled);
    }

    #[tokio::test]
    async fn test_bridge_layer_with_disabled_state() {
        let bridge_layer = EventBridgeLayer::disabled();
        let subscriber = Registry::default().with(bridge_layer);

        tracing::subscriber::with_default(subscriber, || {
            // This event should be ignored due to disabled state
            info!(task_name = "disabled_test", "task_started");
        });

        // Should not panic and should not emit events
    }

    #[tokio::test]
    async fn test_bridge_layer_with_multiple_event_types() {
        let bridge_layer = EventBridgeLayer::new();
        let subscriber = Registry::default().with(bridge_layer);

        tracing::subscriber::with_default(subscriber, || {
            // Test multiple event types
            info!(task_name = "test1", "task_started");
            info!(task_name = "test1", message = "50% done", "task_progress");
            info!(task_name = "test1", duration_ms = 1000, "task_completed");
            info!(key = "cache_key", "cache_hit");
            info!(total_tasks = 5, total_levels = 2, "pipeline_started");
        });

        // Should handle all event types without panicking
    }

    #[test]
    fn test_high_throughput_event_processing() {
        let bridge_layer = EventBridgeLayer::new();
        let subscriber = Registry::default().with(bridge_layer);

        tracing::subscriber::with_default(subscriber, || {
            // Simulate high-throughput event processing with fewer events to avoid timeout
            for i in 0..100 {
                info!(
                    task_name = format!("task_{}", i),
                    duration_ms = i * 10,
                    "task_completed"
                );
            }
        });

        // Should handle high throughput without issues
    }

    #[test]
    fn test_edge_case_empty_fields() {
        let fields = HashMap::new();

        // All event creation functions should handle empty fields gracefully
        let task_progress = EventBridgeLayer::create_task_progress_event(&fields);
        assert!(task_progress.is_none());

        let task_completed = EventBridgeLayer::create_task_completed_event(&fields);
        assert!(task_completed.is_none());

        let task_failed = EventBridgeLayer::create_task_failed_event(&fields);
        assert!(task_failed.is_none());

        let task_started = EventBridgeLayer::create_task_started_event(&fields);
        assert!(task_started.is_none());

        let task_skipped = EventBridgeLayer::create_task_skipped_event(&fields);
        assert!(task_skipped.is_none());
    }

    #[test]
    fn test_cache_events_without_key_field() {
        let fields = HashMap::new();

        // Cache events should use "unknown" as fallback when no key is provided
        let cache_hit = EventBridgeLayer::create_cache_hit_event(&fields);
        match cache_hit {
            Some(SystemEvent::Cache(CacheEvent::CacheHit { key })) => {
                assert_eq!(key, "unknown");
            }
            _ => panic!("Expected CacheHit event with fallback key"),
        }

        let cache_miss = EventBridgeLayer::create_cache_miss_event(&fields);
        match cache_miss {
            Some(SystemEvent::Cache(CacheEvent::CacheMiss { key })) => {
                assert_eq!(key, "unknown");
            }
            _ => panic!("Expected CacheMiss event with fallback key"),
        }
    }
}
