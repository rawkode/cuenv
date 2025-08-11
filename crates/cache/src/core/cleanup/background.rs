//! Background cleanup task management

use std::sync::Arc;
use std::time::Duration;

use crate::core::types::Cache;

/// Start the background cleanup task
pub fn start_cleanup_task(cache: &Cache) {
    let inner = Arc::clone(&cache.inner);
    let cleanup_interval = inner.config.cleanup_interval;

    // Don't start cleanup task if interval is zero (useful for tests)
    if cleanup_interval == Duration::ZERO {
        return;
    }

    let inner_clone = Arc::clone(&inner);
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(cleanup_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match Cache::cleanup_expired_entries(&inner_clone).await {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!("Cache cleanup error: {}", e);
                }
            }
        }
    });

    *cache.inner.cleanup_handle.write() = Some(handle);
}
