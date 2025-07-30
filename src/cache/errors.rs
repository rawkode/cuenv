//! Production-grade error handling for the cache system
//!
//! This module provides comprehensive error types with recovery strategies
//! and detailed context for debugging and operational monitoring.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// Result type for cache operations
pub type Result<T> = std::result::Result<T, CacheError>;

/// Comprehensive error type for cache operations
#[derive(Debug)]
pub enum CacheError {
    /// I/O errors during cache operations
    Io {
        path: PathBuf,
        operation: &'static str,
        source: std::io::Error,
        recovery_hint: RecoveryHint,
    },

    /// Serialization/deserialization errors
    Serialization {
        key: String,
        operation: SerializationOp,
        source: Box<dyn std::error::Error + Send + Sync>,
        recovery_hint: RecoveryHint,
    },

    /// Cache corruption detected
    Corruption {
        key: String,
        reason: String,
        recovery_hint: RecoveryHint,
    },

    /// Cache capacity exceeded
    CapacityExceeded {
        requested_bytes: u64,
        available_bytes: u64,
        recovery_hint: RecoveryHint,
    },

    /// Concurrent access conflict
    ConcurrencyConflict {
        key: String,
        operation: &'static str,
        duration: Duration,
        recovery_hint: RecoveryHint,
    },

    /// Invalid cache key
    InvalidKey {
        key: String,
        reason: String,
        recovery_hint: RecoveryHint,
    },

    /// Cache store unavailable
    StoreUnavailable {
        store_type: StoreType,
        reason: String,
        recovery_hint: RecoveryHint,
    },

    /// Version mismatch in cached data
    VersionMismatch {
        key: String,
        expected_version: u32,
        actual_version: u32,
        recovery_hint: RecoveryHint,
    },

    /// Permission denied
    PermissionDenied {
        path: PathBuf,
        operation: &'static str,
        recovery_hint: RecoveryHint,
    },

    /// Network error (for remote cache)
    Network {
        endpoint: String,
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
        recovery_hint: RecoveryHint,
    },

    /// Timeout during cache operation
    Timeout {
        operation: &'static str,
        duration: Duration,
        recovery_hint: RecoveryHint,
    },

    /// Disk quota exceeded
    DiskQuotaExceeded {
        current: u64,
        requested: u64,
        limit: u64,
        recovery_hint: RecoveryHint,
    },

    /// Integrity check failed
    IntegrityFailure {
        key: String,
        expected_hash: String,
        actual_hash: String,
        recovery_hint: RecoveryHint,
    },

    /// Configuration error
    Configuration {
        message: String,
        recovery_hint: RecoveryHint,
    },

    /// Compression/decompression error
    Compression {
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
        recovery_hint: RecoveryHint,
    },
}

/// Recovery hints for error handling
#[derive(Debug, Clone)]
pub enum RecoveryHint {
    /// Retry the operation
    Retry { after: Duration },
    /// Clear the cache and retry
    ClearAndRetry,
    /// Increase cache capacity
    IncreaseCapacity { suggested_bytes: u64 },
    /// Check file permissions
    CheckPermissions { path: PathBuf },
    /// Verify network connectivity
    CheckNetwork { endpoint: String },
    /// Rebuild cache index
    RebuildIndex,
    /// No automated recovery possible
    Manual { instructions: String },
    /// Operation can be safely ignored
    Ignore,
    /// Use a default value
    UseDefault { value: String },
    /// Check available disk space
    CheckDiskSpace,
    /// Run cache eviction
    RunEviction,
}

/// Serialization operation type
#[derive(Debug)]
pub enum SerializationOp {
    Encode,
    Decode,
}

/// Cache store type
#[derive(Debug)]
pub enum StoreType {
    Local,
    Remote { endpoint: String },
    ContentAddressed,
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                path,
                operation,
                source,
                ..
            } => write!(
                f,
                "I/O error during {} on '{}': {}",
                operation,
                path.display(),
                source
            ),
            Self::Serialization {
                key,
                operation,
                source,
                ..
            } => write!(
                f,
                "Failed to {:?} cache entry '{}': {}",
                operation, key, source
            ),
            Self::Corruption { key, reason, .. } => {
                write!(f, "Cache corruption detected for key '{}': {}", key, reason)
            }
            Self::CapacityExceeded {
                requested_bytes,
                available_bytes,
                ..
            } => write!(
                f,
                "Cache capacity exceeded: requested {} bytes, only {} bytes available",
                requested_bytes, available_bytes
            ),
            Self::ConcurrencyConflict {
                key,
                operation,
                duration,
                ..
            } => write!(
                f,
                "Concurrency conflict for key '{}' during {} (waited {:?})",
                key, operation, duration
            ),
            Self::InvalidKey { key, reason, .. } => {
                write!(f, "Invalid cache key '{}': {}", key, reason)
            }
            Self::StoreUnavailable {
                store_type, reason, ..
            } => write!(f, "Cache store {:?} unavailable: {}", store_type, reason),
            Self::VersionMismatch {
                key,
                expected_version,
                actual_version,
                ..
            } => write!(
                f,
                "Version mismatch for key '{}': expected v{}, found v{}",
                key, expected_version, actual_version
            ),
            Self::PermissionDenied {
                path, operation, ..
            } => write!(
                f,
                "Permission denied for {} on '{}'",
                operation,
                path.display()
            ),
            Self::Network {
                endpoint,
                operation,
                source,
                ..
            } => write!(
                f,
                "Network error during {} with '{}': {}",
                operation, endpoint, source
            ),
            Self::Timeout {
                operation,
                duration,
                ..
            } => write!(f, "Timeout during {} after {:?}", operation, duration),
            Self::DiskQuotaExceeded {
                current,
                requested,
                limit,
                ..
            } => write!(
                f,
                "Disk quota exceeded: current {}, requested {}, limit {}",
                current, requested, limit
            ),
            Self::IntegrityFailure {
                key,
                expected_hash,
                actual_hash,
                ..
            } => write!(
                f,
                "Integrity check failed for key '{}': expected hash {}, got {}",
                key, expected_hash, actual_hash
            ),
            Self::Configuration { message, .. } => {
                write!(f, "Cache configuration error: {}", message)
            }
            Self::Compression {
                operation, source, ..
            } => write!(f, "Compression error during {}: {}", operation, source),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Serialization { source, .. } => Some(source.as_ref()),
            Self::Network { source, .. } => Some(source.as_ref()),
            Self::Compression { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl CacheError {
    /// Get the recovery hint for this error
    #[must_use]
    pub const fn recovery_hint(&self) -> &RecoveryHint {
        match self {
            Self::Io { recovery_hint, .. }
            | Self::Serialization { recovery_hint, .. }
            | Self::Corruption { recovery_hint, .. }
            | Self::CapacityExceeded { recovery_hint, .. }
            | Self::ConcurrencyConflict { recovery_hint, .. }
            | Self::InvalidKey { recovery_hint, .. }
            | Self::StoreUnavailable { recovery_hint, .. }
            | Self::VersionMismatch { recovery_hint, .. }
            | Self::PermissionDenied { recovery_hint, .. }
            | Self::Network { recovery_hint, .. }
            | Self::Timeout { recovery_hint, .. }
            | Self::DiskQuotaExceeded { recovery_hint, .. }
            | Self::IntegrityFailure { recovery_hint, .. }
            | Self::Configuration { recovery_hint, .. }
            | Self::Compression { recovery_hint, .. } => recovery_hint,
        }
    }

    /// Check if this error is transient and can be retried
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(
            self.recovery_hint(),
            RecoveryHint::Retry { .. } | RecoveryHint::CheckNetwork { .. }
        )
    }

    /// Check if this error indicates data corruption
    #[must_use]
    pub const fn is_corruption(&self) -> bool {
        matches!(
            self,
            Self::Corruption { .. } | Self::IntegrityFailure { .. }
        )
    }
}

/// Error conversion utilities
impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;

        let recovery_hint = match error.kind() {
            ErrorKind::PermissionDenied => RecoveryHint::CheckPermissions {
                path: PathBuf::from("."),
            },
            ErrorKind::NotFound => RecoveryHint::Ignore,
            ErrorKind::WouldBlock | ErrorKind::TimedOut => RecoveryHint::Retry {
                after: Duration::from_millis(100),
            },
            _ => RecoveryHint::Manual {
                instructions: "Check system logs for details".to_string(),
            },
        };

        Self::Io {
            path: PathBuf::from("."),
            operation: "unknown",
            source: error,
            recovery_hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = CacheError::InvalidKey {
            key: "test/key".to_string(),
            reason: "contains invalid characters".to_string(),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Use alphanumeric characters only".to_string(),
            },
        };

        assert_eq!(
            error.to_string(),
            "Invalid cache key 'test/key': contains invalid characters"
        );
    }

    #[test]
    fn test_error_transient_check() {
        let transient_error = CacheError::Network {
            endpoint: "http://cache.example.com".to_string(),
            operation: "fetch",
            source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
            recovery_hint: RecoveryHint::Retry {
                after: Duration::from_secs(1),
            },
        };

        assert!(transient_error.is_transient());

        let permanent_error = CacheError::Corruption {
            key: "key".to_string(),
            reason: "checksum mismatch".to_string(),
            recovery_hint: RecoveryHint::ClearAndRetry,
        };

        assert!(!permanent_error.is_transient());
    }
}
