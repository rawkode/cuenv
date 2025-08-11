//! Core eviction policy trait definition

/// Eviction policy trait
pub trait EvictionPolicy: Send + Sync {
    /// Record access to a key
    fn on_access(&self, key: &str, size: u64);

    /// Record insertion of a key
    fn on_insert(&self, key: &str, size: u64);

    /// Record removal of a key
    fn on_remove(&self, key: &str, size: u64);

    /// Get next key to evict
    fn next_eviction(&self) -> Option<String>;

    /// Clear all tracking data
    fn clear(&self);

    /// Get current memory usage
    fn memory_usage(&self) -> u64;
}
