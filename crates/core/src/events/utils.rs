//! Utility functions for common event patterns

use crate::events::types::{CacheEvent, SystemEvent, TaskEvent};

/// Create a task started event
pub fn task_started(task_name: &str, task_id: &str) -> SystemEvent {
    SystemEvent::Task(TaskEvent::TaskStarted {
        task_name: task_name.to_string(),
        task_id: task_id.to_string(),
    })
}

/// Create a task completed event
pub fn task_completed(task_name: &str, task_id: &str, duration_ms: u64) -> SystemEvent {
    SystemEvent::Task(TaskEvent::TaskCompleted {
        task_name: task_name.to_string(),
        task_id: task_id.to_string(),
        duration_ms,
    })
}

/// Create a task failed event
pub fn task_failed(task_name: &str, task_id: &str, error: &str) -> SystemEvent {
    SystemEvent::Task(TaskEvent::TaskFailed {
        task_name: task_name.to_string(),
        task_id: task_id.to_string(),
        error: error.to_string(),
    })
}

/// Create a cache hit event
pub fn cache_hit(key: &str) -> SystemEvent {
    SystemEvent::Cache(CacheEvent::CacheHit {
        key: key.to_string(),
    })
}

/// Create a cache miss event
pub fn cache_miss(key: &str) -> SystemEvent {
    SystemEvent::Cache(CacheEvent::CacheMiss {
        key: key.to_string(),
    })
}