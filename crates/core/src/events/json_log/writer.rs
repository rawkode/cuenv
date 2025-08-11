//! File writer for JSON log operations

use super::error::JsonLogError;
use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;

/// Log file writer with buffering
pub struct LogWriter {
    /// Buffered writer
    writer: Mutex<Option<BufWriter<File>>>,
}

impl LogWriter {
    /// Create a new log writer
    pub async fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, JsonLogError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await
            .map_err(|e| JsonLogError::IoError(format!("Failed to open log file: {e}")))?;

        let buf_writer = BufWriter::new(file);
        Ok(Self {
            writer: Mutex::new(Some(buf_writer)),
        })
    }

    /// Write content to the log file
    pub async fn write(&self, content: &str) -> Result<usize, JsonLogError> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            let content_bytes = content.as_bytes();
            writer
                .write_all(content_bytes)
                .await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
            writer
                .write_all(b"\n")
                .await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
            writer
                .flush()
                .await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;

            // Return bytes written including newline
            Ok(content_bytes.len() + 1)
        } else {
            Err(JsonLogError::IoError("Writer not initialized".to_string()))
        }
    }

    /// Flush pending writes
    pub async fn flush(&self) -> Result<(), JsonLogError> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer
                .flush()
                .await
                .map_err(|e| JsonLogError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    /// Close the writer and flush remaining data
    pub async fn close(&self) -> Result<(), JsonLogError> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(mut buf_writer) = writer_guard.take() {
            if let Err(e) = buf_writer.flush().await {
                tracing::warn!("Failed to flush log buffer during close: {}", e);
            }
        }
        Ok(())
    }

    /// Reinitialize the writer with a new file
    pub async fn reinitialize<P: AsRef<Path>>(&self, file_path: P) -> Result<(), JsonLogError> {
        // Close existing writer
        self.close().await?;

        // Open new file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await
            .map_err(|e| JsonLogError::IoError(format!("Failed to open log file: {e}")))?;

        let buf_writer = BufWriter::new(file);
        *self.writer.lock().await = Some(buf_writer);
        Ok(())
    }
}