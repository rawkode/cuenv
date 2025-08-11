//! Cache-related events

use serde::{Deserialize, Serialize};

/// Cache-related events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheEvent {
    /// Cache hit for a task
    CacheHit { key: String },
    /// Cache miss for a task
    CacheMiss { key: String },
    /// Cache entry written
    CacheWrite { key: String, size_bytes: u64 },
    /// Cache entry evicted
    CacheEvict { key: String, reason: String },
}