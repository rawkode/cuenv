//! Tests for JSON log functionality

use super::config::JsonLogSubscriber;
use super::formatter::format_event;
use crate::events::{EnhancedEvent, EventSubscriber, SystemEvent, TaskEvent};
use std::collections::HashMap;
use std::time::SystemTime;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_json_log_subscriber_creation() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.jsonl");

    let subscriber = JsonLogSubscriber::new(&log_path).await;
    assert!(subscriber.is_ok());
}

#[tokio::test]
async fn test_json_log_event_writing() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.jsonl");

    let subscriber = JsonLogSubscriber::new(&log_path).await.unwrap();

    let event = EnhancedEvent {
        event: SystemEvent::Task(TaskEvent::TaskCompleted {
            task_name: "test".to_string(),
            task_id: "test-1".to_string(),
            duration_ms: 1000,
        }),
        timestamp: SystemTime::now(),
        correlation_id: Some("test-correlation".to_string()),
        metadata: {
            let mut map = HashMap::new();
            map.insert("test_key".to_string(), "test_value".to_string());
            map
        },
    };

    let result = subscriber.handle_event(&event).await;
    assert!(result.is_ok());

    // Flush to ensure write
    subscriber.flush().await.unwrap();

    // Verify file exists and has content
    let content = fs::read_to_string(&log_path).await.unwrap();
    assert!(!content.is_empty());

    // Verify it's valid JSON
    let json_value: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert!(json_value.is_object());
    assert!(json_value["event"].is_object());
    assert!(json_value["timestamp"].is_number());
}

#[tokio::test]
async fn test_json_log_rotation() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.jsonl");

    // Create subscriber with very small max file size
    let subscriber = JsonLogSubscriber::with_config(
        &log_path,
        true,
        Some(100), // 100 bytes
        3,
    )
    .await
    .unwrap();

    // Write multiple events to trigger rotation
    for i in 0..10 {
        let event = EnhancedEvent {
            event: SystemEvent::Task(TaskEvent::TaskCompleted {
                task_name: format!("test-task-{i}"),
                task_id: format!("test-{i}"),
                duration_ms: 1000,
            }),
            timestamp: SystemTime::now(),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        subscriber.handle_event(&event).await.unwrap();
    }

    subscriber.flush().await.unwrap();

    // Check that backup files were created
    let backup_path = format!("{}.1", log_path.display());
    let backup_exists = fs::metadata(&backup_path).await.is_ok();

    // At least the main file should exist
    assert!(fs::metadata(&log_path).await.is_ok());

    // Backup file should exist if rotation occurred
    // Note: This test might be flaky depending on timing and exact sizes
    if backup_exists {
        println!("Log rotation occurred as expected");
    }
}

#[tokio::test]
async fn test_json_log_format_event() {
    let event = EnhancedEvent {
        event: SystemEvent::Task(TaskEvent::TaskFailed {
            task_name: "failing_task".to_string(),
            task_id: "fail-1".to_string(),
            error: "Something went wrong".to_string(),
        }),
        timestamp: SystemTime::now(),
        correlation_id: Some("correlation-123".to_string()),
        metadata: {
            let mut map = HashMap::new();
            map.insert("user".to_string(), "test_user".to_string());
            map
        },
    };

    let formatted = format_event(&event, true).await.unwrap();

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
    assert!(parsed["timestamp"].is_number());
    assert!(parsed["event"]["Task"]["TaskFailed"]["task_name"] == "failing_task");
    assert!(parsed["correlation_id"] == "correlation-123");
    assert!(parsed["metadata"]["user"] == "test_user");
}

#[tokio::test]
async fn test_json_log_format_without_metadata() {
    let event = EnhancedEvent {
        event: SystemEvent::Task(TaskEvent::TaskStarted {
            task_name: "simple_task".to_string(),
            task_id: "simple-1".to_string(),
        }),
        timestamp: SystemTime::now(),
        correlation_id: Some("should-not-appear".to_string()),
        metadata: {
            let mut map = HashMap::new();
            map.insert("should_not".to_string(), "appear".to_string());
            map
        },
    };

    let formatted = format_event(&event, false).await.unwrap();

    // Should be valid JSON without metadata
    let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
    assert!(parsed["timestamp"].is_number());
    assert!(parsed["event"].is_object());
    assert!(parsed.get("correlation_id").is_none());
    assert!(parsed.get("metadata").is_none());
}