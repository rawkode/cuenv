//! Shared types for monitoring module

use crate::errors::CacheError;

pub use super::analyzer::HitRateReport;

impl CacheError {
    /// Get error type for metrics
    #[allow(dead_code)]
    pub fn error_type(&self) -> &'static str {
        match self {
            CacheError::InvalidKey { .. } => "invalid_key",
            CacheError::Serialization { .. } => "serialization",
            CacheError::Corruption { .. } => "corruption",
            CacheError::Io { .. } => "io",
            CacheError::CapacityExceeded { .. } => "capacity_exceeded",
            CacheError::Configuration { .. } => "configuration",
            CacheError::StoreUnavailable { .. } => "store_unavailable",
            CacheError::ConcurrencyConflict { .. } => "concurrency_conflict",
            CacheError::Network { .. } => "remote_error",
            CacheError::Timeout { .. } => "timeout",
            _ => "unknown",
        }
    }
}

/// Size bucket for metrics
#[allow(dead_code)]
pub fn size_bucket(size_bytes: u64) -> &'static str {
    match size_bytes {
        0..=1024 => "small",
        1025..=65536 => "medium",
        65537..=1048576 => "large",
        _ => "xlarge",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_bucket() {
        assert_eq!(size_bucket(512), "small");
        assert_eq!(size_bucket(32768), "medium");
        assert_eq!(size_bucket(524288), "large");
        assert_eq!(size_bucket(2097152), "xlarge");
    }
}
