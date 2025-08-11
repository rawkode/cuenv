//! Shared types for monitoring module

use crate::errors::CacheError;

pub use super::analyzer::HitRateReport;

impl CacheError {
    /// Get error type for metrics
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
