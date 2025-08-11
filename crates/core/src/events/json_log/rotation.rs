//! Log file rotation logic

use super::config::JsonLogSubscriber;
use super::error::JsonLogError;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

impl JsonLogSubscriber {
    /// Check if log rotation is needed and perform it (optimized with caching)
    pub(crate) async fn check_rotation(&self) -> Result<(), JsonLogError> {
        let Some(max_size) = self.config.max_file_size else {
            return Ok(());
        };

        // Check every size_check_interval writes or when cached size exceeds threshold
        let write_count = self.write_counter.fetch_add(1, Ordering::Relaxed);
        let cached_size = self.cached_file_size.load(Ordering::Relaxed);

        let should_check_size =
            write_count % self.config.size_check_interval == 0 || cached_size > max_size;

        if should_check_size {
            // Check actual file size
            let actual_size = match tokio::fs::metadata(&self.config.file_path).await {
                Ok(meta) => meta.len(),
                Err(_) => {
                    // File doesn't exist yet, reset cache
                    self.cached_file_size.store(0, Ordering::Relaxed);
                    return Ok(());
                }
            };

            // Update cached size
            self.cached_file_size.store(actual_size, Ordering::Relaxed);

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
        self.writer.close().await?;

        // Move existing backup files
        for i in (1..self.config.backup_count).rev() {
            let old_path = format!("{}.{}", self.config.file_path.display(), i);
            let new_path = format!("{}.{}", self.config.file_path.display(), i + 1);

            if tokio::fs::metadata(&old_path).await.is_ok() {
                if let Err(e) = tokio::fs::rename(&old_path, &new_path).await {
                    warn!(
                        "Failed to rotate log file {} to {}: {}",
                        old_path, new_path, e
                    );
                }
            }
        }

        // Move current log to .1
        let backup_path = format!("{}.1", self.config.file_path.display());
        if let Err(e) = tokio::fs::rename(&self.config.file_path, &backup_path).await {
            warn!("Failed to move current log to backup: {}", e);
        }

        // Recreate writer with new file
        self.writer.reinitialize(&self.config.file_path).await?;

        // Reset cached file size after rotation
        self.cached_file_size.store(0, Ordering::Relaxed);
        self.write_counter.store(0, Ordering::Relaxed);

        debug!("Log rotation completed");
        Ok(())
    }
}
