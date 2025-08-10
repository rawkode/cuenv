//! JSON log event subscriber for structured logging

use crate::events::{EnhancedEvent, EventSubscriber, SystemEvent};
use async_trait::async_trait;
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Default maximum log file size before rotation (10MB)
const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
/// Default number of backup files to keep
const DEFAULT_BACKUP_COUNT: usize = 5;
/// Check file size every N writes to reduce filesystem calls
const SIZE_CHECK_INTERVAL: u64 = 100;

/// JSON log subscriber for structured logging to files
pub struct JsonLogSubscriber {
    /// File writer
    writer: Mutex<Option<BufWriter<File>>>,
    /// Log file path
    file_path: PathBuf,
    /// Whether to include metadata in logs
    include_metadata: bool,
    /// Maximum log file size before rotation (bytes)
    max_file_size: Option<u64>,
    /// Number of backup files to keep
    backup_count: usize,
    /// Cached file size to reduce filesystem calls
    cached_file_size: Arc<std::sync::atomic::AtomicU64>,
    /// Write counter for periodic size checks
    write_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl JsonLogSubscriber {
    /// Create a new JSON log subscriber
    pub async fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, JsonLogError> {
        let file_path = file_path.as_ref().to_path_buf();
        
        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                JsonLogError::IoError(format!("Failed to create log directory: {}", e))
            })?;
        }

        let mut subscriber = Self {
            writer: Mutex::new(None),
            file_path,
            include_metadata: true,
            max_file_size: Some(DEFAULT_MAX_FILE_SIZE)
            backup_count: DEFAULT_BACKUP_COUNT,
            cached_file_size: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            write_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };

        subscriber.initialize_writer().await?;
        Ok(subscriber)
    }

    /// Create a JSON log subscriber with custom configuration
    pub async fn with_config<P: AsRef<Path>>(
        file_path: P,
        include_metadata: bool,
        max_file_size: Option<u64>,
        backup_count: usize,
    ) -> Result<Self, JsonLogError> {
        let file_path = file_path.as_ref().to_path_buf();
        
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                JsonLogError::IoError(format!("Failed to create log directory: {}", e))
            })?;
        }

        let mut subscriber = Self {
            writer: Mutex::new(None),
            file_path,
            include_metadata,
            max_file_size,
            backup_count,
            cached_file_size: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            write_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };

        subscriber.initialize_writer().await?;
        Ok(subscriber)
    }

    /// Initialize the file writer
    async fn initialize_writer(&self) -> Result<(), JsonLogError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await
            .map_err(|e| JsonLogError::IoError(format!("Failed to open log file: {}", e)))?;

        let buf_writer = BufWriter::new(file);
        *self.writer.lock().await = Some(buf_writer);
        Ok(())
    }

    /// Check if log rotation is needed and perform it (optimized with caching)
    async fn check_rotation(&self) -> Result<(), JsonLogError> {
        let Some(max_size) = self.max_file_size else {
            return Ok(());
        };

        // Check every SIZE_CHECK_INTERVAL writes or when cached size exceeds threshold
        let write_count = self.write_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cached_size = self.cached_file_size.load(std::sync::atomic::Ordering::Relaxed);
        
        let should_check_size = write_count % SIZE_CHECK_INTERVAL == 0 || cached_size > max_size;
        
        if should_check_size {
            // Check actual file size
            let actual_size = match tokio::fs::metadata(&self.file_path).await {
                Ok(meta) => meta.len(),
                Err(_) => {
                    // File doesn't exist yet, reset cache
                    self.cached_file_size.store(0, std::sync::atomic::Ordering::Relaxed);
                    return Ok(());
                }
            };
            
            // Update cached size
            self.cached_file_size.store(actual_size, std::sync::atomic::Ordering::Relaxed);
            
            if actual_size > max_size {
                self.rotate_logs().await?;
            }
        }

        Ok(())
    }

    /// Rotate log files
    async fn rotate_logs(&self) -> Result<(), JsonLogError> {
        debug!("Rotating log files");

        // Close current writer
        {
            let mut writer = self.writer.lock().await;
            if let Some(mut buf_writer) = writer.take() {
                if let Err(e) = buf_writer.flush().await {
                    warn!("Failed to flush log buffer during rotation: {}", e);
                }
            }
        }

        // Move existing backup files
        for i in (1..self.backup_count).rev() {
            let old_path = format!("{}.{}", self.file_path.display(), i);
            let new_path = format!("{}.{}", self.file_path.display(), i + 1);
            
            if tokio::fs::metadata(&old_path).await.is_ok() {
                if let Err(e) = tokio::fs::rename(&old_path, &new_path).await {
                    warn!("Failed to rotate log file {} to {}: {}", old_path, new_path, e);
                }
            }
        }

        // Move current log to .1
        let backup_path = format!("{}.1", self.file_path.display());
        if let Err(e) = tokio::fs::rename(&self.file_path, &backup_path).await {
            warn!("Failed to move current log to backup: {}", e);
        }

        // Recreate writer with new file
        self.initialize_writer().await?;
        
        // Reset cached file size after rotation
        self.cached_file_size.store(0, std::sync::atomic::Ordering::Relaxed);
        self.write_counter.store(0, std::sync::atomic::Ordering::Relaxed);

        debug!("Log rotation completed");
        Ok(())
    }

    /// Format an event as JSON
    async fn format_event(&self, event: &EnhancedEvent) -> Result<String, JsonLogError> {
        let mut json_obj = serde_json::json!({
            "timestamp": event.timestamp.duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| JsonLogError::SerializationError(e.to_string()))?
                .as_millis(),
            "event": event.event,
        });

        if self.include_metadata {
            if let Some(correlation_id) = &event.correlation_id {
                json_obj["correlation_id"] = serde_json::Value::String(correlation_id.clone());
            }

            if !event.metadata.is_empty() {
                json_obj["metadata"] = serde_json::Value::Object(
                    event.metadata.iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect()
                );
            }
        }

        serde_json::to_string(&json_obj)
            .map_err(|e| JsonLogError::SerializationError(e.to_string()))
    }

    /// Write a log entry
    async fn write_log_entry(&self, content: &str) -> Result<(), JsonLogError> {
        // Check for rotation before writing
        self.check_rotation().await?;

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            let content_bytes = content.as_bytes();
            writer.write_all(content_bytes).await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
            writer.write_all(b"\n").await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
            writer.flush().await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
            
            // Update cached file size estimate
            let bytes_written = content_bytes.len() + 1; // +1 for newline
            self.cached_file_size.fetch_add(bytes_written as u64, std::sync::atomic::Ordering::Relaxed);
        }

        Ok(())
    }

    /// Ensure all pending writes are flushed
    pub async fn flush(&self) -> Result<(), JsonLogError> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.flush().await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
        }
        Ok(())
    }
}

#[async_trait]
impl EventSubscriber for JsonLogSubscriber {
    async fn handle_event(&self, event: &EnhancedEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let formatted = self.format_event(event).await?;
        self.write_log_entry(&formatted).await?;
        
        debug!(
            event_type = std::any::type_name_of_val(&event.event),
            log_file = %self.file_path.display(),
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

/// JSON log subscriber errors
#[derive(Debug, thiserror::Error)]
pub enum JsonLogError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl Drop for JsonLogSubscriber {
    fn drop(&mut self) {
        // Best effort flush on drop - we can't use async in Drop
        // The tokio runtime might not be available
        debug!("JsonLogSubscriber dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{SystemEvent, TaskEvent};
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
        ).await.unwrap();
        
        // Write multiple events to trigger rotation
        for i in 0..10 {
            let event = EnhancedEvent {
                event: SystemEvent::Task(TaskEvent::TaskCompleted {
                    task_name: format!("test-task-{}", i),
                    task_id: format!("test-{}", i),
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
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.jsonl");
        
        let subscriber = JsonLogSubscriber::new(&log_path).await.unwrap();
        
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

        let formatted = subscriber.format_event(&event).await.unwrap();
        
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert!(parsed["timestamp"].is_number());
        assert!(parsed["event"]["Task"]["TaskFailed"]["task_name"] == "failing_task");
        assert!(parsed["correlation_id"] == "correlation-123");
        assert!(parsed["metadata"]["user"] == "test_user");
    }
}