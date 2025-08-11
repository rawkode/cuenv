//! Configuration and builder for JSON log subscriber

use super::error::JsonLogError;
use super::writer::LogWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default maximum log file size before rotation (10MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
/// Default number of backup files to keep
pub const DEFAULT_BACKUP_COUNT: usize = 5;
/// Check file size every N writes to reduce filesystem calls
pub const SIZE_CHECK_INTERVAL: u64 = 100;

/// Configuration for JSON log subscriber
#[derive(Debug, Clone)]
pub struct JsonLogConfig {
    /// Log file path
    pub file_path: PathBuf,
    /// Whether to include metadata in logs
    pub include_metadata: bool,
    /// Maximum log file size before rotation (bytes)
    pub max_file_size: Option<u64>,
    /// Number of backup files to keep
    pub backup_count: usize,
    /// Check file size every N writes (configurable to reduce filesystem calls)
    pub size_check_interval: u64,
}

impl JsonLogConfig {
    /// Create a new configuration with default values
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            include_metadata: true,
            max_file_size: Some(DEFAULT_MAX_FILE_SIZE),
            backup_count: DEFAULT_BACKUP_COUNT,
            size_check_interval: SIZE_CHECK_INTERVAL,
        }
    }

    /// Set whether to include metadata
    pub fn with_metadata(mut self, include_metadata: bool) -> Self {
        self.include_metadata = include_metadata;
        self
    }

    /// Set maximum file size before rotation
    pub fn with_max_file_size(mut self, max_file_size: Option<u64>) -> Self {
        self.max_file_size = max_file_size;
        self
    }

    /// Set number of backup files to keep
    pub fn with_backup_count(mut self, backup_count: usize) -> Self {
        self.backup_count = backup_count;
        self
    }

    /// Set size check interval
    pub fn with_size_check_interval(mut self, size_check_interval: u64) -> Self {
        self.size_check_interval = size_check_interval;
        self
    }
}

/// JSON log subscriber for structured logging to files
pub struct JsonLogSubscriber {
    /// Configuration
    pub(crate) config: JsonLogConfig,
    /// File writer
    pub(crate) writer: LogWriter,
    /// Cached file size to reduce filesystem calls
    pub(crate) cached_file_size: Arc<std::sync::atomic::AtomicU64>,
    /// Write counter for periodic size checks
    pub(crate) write_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl JsonLogSubscriber {
    /// Create a new JSON log subscriber
    pub async fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, JsonLogError> {
        let config = JsonLogConfig::new(file_path);
        Self::from_config(config).await
    }

    /// Create a JSON log subscriber with custom configuration
    pub async fn with_config<P: AsRef<Path>>(
        file_path: P,
        include_metadata: bool,
        max_file_size: Option<u64>,
        backup_count: usize,
    ) -> Result<Self, JsonLogError> {
        let config = JsonLogConfig::new(file_path)
            .with_metadata(include_metadata)
            .with_max_file_size(max_file_size)
            .with_backup_count(backup_count);
        Self::from_config(config).await
    }

    /// Create a JSON log subscriber with custom size check interval (for performance tuning)
    pub async fn with_size_check_interval<P: AsRef<Path>>(
        file_path: P,
        size_check_interval: u64,
    ) -> Result<Self, JsonLogError> {
        let config = JsonLogConfig::new(file_path).with_size_check_interval(size_check_interval);
        Self::from_config(config).await
    }

    /// Create from configuration
    pub async fn from_config(config: JsonLogConfig) -> Result<Self, JsonLogError> {
        // Ensure parent directory exists
        if let Some(parent) = config.file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                JsonLogError::IoError(format!("Failed to create log directory: {e}"))
            })?;
        }

        let writer = LogWriter::new(&config.file_path).await?;

        Ok(Self {
            config,
            writer,
            cached_file_size: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            write_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Ensure all pending writes are flushed
    pub async fn flush(&self) -> Result<(), JsonLogError> {
        self.writer.flush().await
    }
}

impl Drop for JsonLogSubscriber {
    fn drop(&mut self) {
        // Best effort flush on drop - we can't use async in Drop
        // The tokio runtime might not be available
        tracing::debug!("JsonLogSubscriber dropped");
    }
}