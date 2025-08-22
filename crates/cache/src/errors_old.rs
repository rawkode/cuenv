//! Production-grade error handling for the cache system
//!
//! This module provides comprehensive error types with recovery strategies
//! and detailed context for debugging and operational monitoring.

// Import from the modular error system
mod conversions;
mod display;
mod recovery;
mod security;
mod types;

// Re-export everything for backward compatibility
pub use conversions::*;
pub use display::*;
pub use recovery::*;
pub use security::*;
pub use types::*;

/// Result type for cache operations
pub type Result<T> = std::result::Result<T, CacheError>;

/// Re-export CacheError as Error for backward compatibility
pub use CacheError as Error;

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

    /// Security-related errors (Phase 7)

    /// Cryptographic signature verification failed
    SignatureVerification {
        algorithm: String,
        key_id: String,
        reason: String,
        recovery_hint: RecoveryHint,
    },

    /// Access denied due to insufficient capabilities
    AccessDenied {
        operation: String,
        required_permission: String,
        token_id: String,
        recovery_hint: RecoveryHint,
    },

    /// Security token is invalid or expired
    InvalidToken {
        token_id: String,
        reason: TokenInvalidReason,
        recovery_hint: RecoveryHint,
    },

    /// Audit log corruption or tampering detected
    AuditLogCorruption {
        log_file: PathBuf,
        corruption_type: AuditCorruptionType,
        recovery_hint: RecoveryHint,
    },

    /// Merkle tree integrity verification failed
    MerkleTreeCorruption {
        root_hash: String,
        expected_hash: String,
        corrupted_entries: Vec<String>,
        recovery_hint: RecoveryHint,
    },

    /// Rate limiting exceeded
    RateLimitExceeded {
        token_id: String,
        limit: f64,
        window_seconds: u64,
        recovery_hint: RecoveryHint,
    },

    /// Security policy violation
    SecurityPolicyViolation {
        policy_name: String,
        violation_details: String,
        severity: ViolationSeverity,
        recovery_hint: RecoveryHint,
    },

    /// Cryptographic key derivation or generation failed
    CryptographicError {
        operation: String,
        algorithm: String,
        details: String,
        recovery_hint: RecoveryHint,
    },

    /// Corruption is unrecoverable
    CorruptionUnrecoverable {
        key: String,
        recovery_hint: RecoveryHint,
    },

    /// Repair is already in progress
    RepairInProgress {
        key: String,
        recovery_hint: RecoveryHint,
    },

    /// All repair strategies failed
    AllRepairStrategiesFailed {
        key: String,
        recovery_hint: RecoveryHint,
    },

    /// Feature not implemented
    NotImplemented { recovery_hint: RecoveryHint },
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
    /// Regenerate security keys
    RegenerateKeys,
    /// Refresh security token
    RefreshToken,
    /// Contact security administrator
    ContactSecurityAdmin { contact: String },
    /// Enable audit logging
    EnableAuditLogging,
    /// Rebuild Merkle tree
    RebuildMerkleTree,
    /// Review security policies
    ReviewSecurityPolicies,
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

/// Security token invalid reasons (Phase 7)
#[derive(Debug, Clone)]
pub enum TokenInvalidReason {
    /// Token has expired
    Expired,
    /// Token has been revoked
    Revoked,
    /// Token signature is invalid
    InvalidSignature,
    /// Token issuer is not trusted
    UntrustedIssuer,
    /// Token format is malformed
    Malformed,
    /// Token is not yet valid (nbf claim)
    NotYetValid,
    /// Token audience mismatch
    AudienceMismatch,
}

/// Audit log corruption types (Phase 7)
#[derive(Debug, Clone)]
pub enum AuditCorruptionType {
    /// Hash chain is broken
    BrokenHashChain,
    /// Entry signature verification failed
    InvalidEntrySignature,
    /// Timestamp is out of order
    TimestampOutOfOrder,
    /// Entry format is invalid
    InvalidFormat,
    /// File was truncated
    FileTruncated,
    /// Unauthorized modification detected
    UnauthorizedModification,
}

/// Security violation severity levels (Phase 7)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViolationSeverity {
    /// Low severity - informational
    Low,
    /// Medium severity - warning
    Medium,
    /// High severity - requires attention
    High,
    /// Critical severity - immediate action required
    Critical,
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
                "Failed to {operation:?} cache entry '{key}': {source}"
            ),
            Self::Corruption { key, reason, .. } => {
                write!(f, "Cache corruption detected for key '{key}': {reason}")
            }
            Self::CapacityExceeded {
                requested_bytes,
                available_bytes,
                ..
            } => write!(
                f,
                "Cache capacity exceeded: requested {requested_bytes} bytes, only {available_bytes} bytes available"
            ),
            Self::ConcurrencyConflict {
                key,
                operation,
                duration,
                ..
            } => write!(
                f,
                "Concurrency conflict for key '{key}' during {operation} (waited {duration:?})"
            ),
            Self::InvalidKey { key, reason, .. } => {
                write!(f, "Invalid cache key '{key}': {reason}")
            }
            Self::StoreUnavailable {
                store_type, reason, ..
            } => write!(f, "Cache store {store_type:?} unavailable: {reason}"),
            Self::VersionMismatch {
                key,
                expected_version,
                actual_version,
                ..
            } => write!(
                f,
                "Version mismatch for key '{key}': expected v{expected_version}, found v{actual_version}"
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
                "Network error during {operation} with '{endpoint}': {source}"
            ),
            Self::Timeout {
                operation,
                duration,
                ..
            } => write!(f, "Timeout during {operation} after {duration:?}"),
            Self::DiskQuotaExceeded {
                current,
                requested,
                limit,
                ..
            } => write!(
                f,
                "Disk quota exceeded: current {current}, requested {requested}, limit {limit}"
            ),
            Self::IntegrityFailure {
                key,
                expected_hash,
                actual_hash,
                ..
            } => write!(
                f,
                "Integrity check failed for key '{key}': expected hash {expected_hash}, got {actual_hash}"
            ),
            Self::Configuration { message, .. } => {
                write!(f, "Cache configuration error: {message}")
            }
            Self::Compression {
                operation, source, ..
            } => write!(f, "Compression error during {operation}: {source}"),
            Self::SignatureVerification {
                algorithm,
                key_id,
                reason,
                ..
            } => write!(
                f,
                "Signature verification failed for {algorithm} key {key_id}: {reason}"
            ),
            Self::AccessDenied {
                operation,
                required_permission,
                token_id,
                ..
            } => write!(
                f,
                "Access denied for operation '{operation}': requires {required_permission} permission (token: {token_id})"
            ),
            Self::InvalidToken {
                token_id, reason, ..
            } => write!(f, "Invalid token {token_id}: {reason:?}"),
            Self::AuditLogCorruption {
                log_file,
                corruption_type,
                ..
            } => write!(
                f,
                "Audit log corruption in '{}': {corruption_type:?}",
                log_file.display()
            ),
            Self::MerkleTreeCorruption {
                root_hash,
                expected_hash,
                corrupted_entries,
                ..
            } => write!(
                f,
                "Merkle tree corruption: root hash {root_hash} != expected {expected_hash}, {} corrupted entries",
                corrupted_entries.len()
            ),
            Self::RateLimitExceeded {
                token_id,
                limit,
                window_seconds,
                ..
            } => write!(
                f,
                "Rate limit exceeded for token {token_id}: {limit} operations per {window_seconds} seconds"
            ),
            Self::SecurityPolicyViolation {
                policy_name,
                violation_details,
                severity,
                ..
            } => write!(
                f,
                "Security policy '{policy_name}' violation ({severity:?}): {violation_details}"
            ),
            Self::CryptographicError {
                operation,
                algorithm,
                details,
                ..
            } => write!(
                f,
                "Cryptographic error during {operation} with {algorithm}: {details}"
            ),
            Self::CorruptionUnrecoverable { key, .. } => {
                write!(f, "Corruption unrecoverable for key: {key}")
            }
            Self::RepairInProgress { key, .. } => write!(f, "Repair in progress for key: {key}"),
            Self::AllRepairStrategiesFailed { key, .. } => {
                write!(f, "All repair strategies failed for key: {key}")
            }
            Self::NotImplemented { .. } => write!(f, "Feature not implemented"),
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
            | Self::Compression { recovery_hint, .. }
            | Self::SignatureVerification { recovery_hint, .. }
            | Self::AccessDenied { recovery_hint, .. }
            | Self::InvalidToken { recovery_hint, .. }
            | Self::AuditLogCorruption { recovery_hint, .. }
            | Self::MerkleTreeCorruption { recovery_hint, .. }
            | Self::RateLimitExceeded { recovery_hint, .. }
            | Self::SecurityPolicyViolation { recovery_hint, .. }
            | Self::CryptographicError { recovery_hint, .. }
            | Self::CorruptionUnrecoverable { recovery_hint, .. }
            | Self::RepairInProgress { recovery_hint, .. }
            | Self::AllRepairStrategiesFailed { recovery_hint, .. }
            | Self::NotImplemented { recovery_hint, .. } => recovery_hint,
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

/// Convert serde_json errors to cache errors
impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization {
            key: String::new(),
            operation: SerializationOp::Decode,
            source: Box::new(error),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check JSON format and data types".to_string(),
            },
        }
    }
}

/// Convert cache errors to core errors
impl From<CacheError> for cuenv_core::Error {
    fn from(error: CacheError) -> Self {
        cuenv_core::Error::configuration(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

    mod error_creation_tests {
        use super::*;

        #[test]
        fn test_io_error_creation() {
            let error = CacheError::Io {
                path: PathBuf::from("/test/path"),
                operation: "read",
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: PathBuf::from("/test/path"),
                },
            };

            assert!(error.to_string().contains("I/O error during read"));
            assert!(error.to_string().contains("/test/path"));
            assert!(error.to_string().contains("file not found"));
        }

        #[test]
        fn test_serialization_error_creation() {
            // Create a real serde error by attempting to deserialize invalid JSON
            let json_error = serde_json::from_str::<String>("invalid json").unwrap_err();

            let error = CacheError::Serialization {
                key: "test-key".to_string(),
                operation: SerializationOp::Encode,
                source: Box::new(json_error),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error.to_string().contains("Failed to Encode"));
            assert!(error.to_string().contains("test-key"));
        }

        #[test]
        fn test_corruption_error_creation() {
            let error = CacheError::Corruption {
                key: "corrupted-key".to_string(),
                reason: "hash mismatch detected".to_string(),
                recovery_hint: RecoveryHint::RebuildIndex,
            };

            assert!(error.to_string().contains("Cache corruption detected"));
            assert!(error.to_string().contains("corrupted-key"));
            assert!(error.to_string().contains("hash mismatch detected"));
        }

        #[test]
        fn test_capacity_exceeded_error_creation() {
            let error = CacheError::CapacityExceeded {
                requested_bytes: 1024,
                available_bytes: 512,
                recovery_hint: RecoveryHint::IncreaseCapacity {
                    suggested_bytes: 2048,
                },
            };

            assert!(error.to_string().contains("Cache capacity exceeded"));
            assert!(error.to_string().contains("1024"));
            assert!(error.to_string().contains("512"));
        }

        #[test]
        fn test_concurrency_conflict_error_creation() {
            let error = CacheError::ConcurrencyConflict {
                key: "conflicted-key".to_string(),
                operation: "write",
                duration: Duration::from_millis(500),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_millis(100),
                },
            };

            assert!(error.to_string().contains("Concurrency conflict"));
            assert!(error.to_string().contains("conflicted-key"));
            assert!(error.to_string().contains("write"));
        }

        #[test]
        fn test_store_unavailable_error_creation() {
            let error = CacheError::StoreUnavailable {
                store_type: StoreType::Remote {
                    endpoint: "cache.example.com".to_string(),
                },
                reason: "connection timeout".to_string(),
                recovery_hint: RecoveryHint::CheckNetwork {
                    endpoint: "cache.example.com".to_string(),
                },
            };

            assert!(error.to_string().contains("Cache store"));
            assert!(error.to_string().contains("unavailable"));
            assert!(error.to_string().contains("connection timeout"));
        }

        #[test]
        fn test_version_mismatch_error_creation() {
            let error = CacheError::VersionMismatch {
                key: "versioned-key".to_string(),
                expected_version: 2,
                actual_version: 1,
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error.to_string().contains("Version mismatch"));
            assert!(error.to_string().contains("versioned-key"));
            assert!(error.to_string().contains("expected v2"));
            assert!(error.to_string().contains("found v1"));
        }

        #[test]
        fn test_permission_denied_error_creation() {
            let error = CacheError::PermissionDenied {
                path: PathBuf::from("/restricted/path"),
                operation: "write",
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: PathBuf::from("/restricted/path"),
                },
            };

            assert!(error.to_string().contains("Permission denied"));
            assert!(error.to_string().contains("write"));
            assert!(error.to_string().contains("/restricted/path"));
        }

        #[test]
        fn test_timeout_error_creation() {
            let error = CacheError::Timeout {
                operation: "cache_read",
                duration: Duration::from_secs(30),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_secs(5),
                },
            };

            assert!(error.to_string().contains("Timeout during cache_read"));
            assert!(error.to_string().contains("30s"));
        }

        #[test]
        fn test_disk_quota_exceeded_error_creation() {
            let error = CacheError::DiskQuotaExceeded {
                current: 1000,
                requested: 500,
                limit: 1200,
                recovery_hint: RecoveryHint::RunEviction,
            };

            assert!(error.to_string().contains("Disk quota exceeded"));
            assert!(error.to_string().contains("current 1000"));
            assert!(error.to_string().contains("requested 500"));
            assert!(error.to_string().contains("limit 1200"));
        }

        #[test]
        fn test_integrity_failure_error_creation() {
            let error = CacheError::IntegrityFailure {
                key: "integrity-key".to_string(),
                expected_hash: "abc123".to_string(),
                actual_hash: "def456".to_string(),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error.to_string().contains("Integrity check failed"));
            assert!(error.to_string().contains("integrity-key"));
            assert!(error.to_string().contains("expected hash abc123"));
            assert!(error.to_string().contains("got def456"));
        }

        #[test]
        fn test_configuration_error_creation() {
            let error = CacheError::Configuration {
                message: "Invalid cache configuration".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check configuration file syntax".to_string(),
                },
            };

            assert!(error.to_string().contains("Cache configuration error"));
            assert!(error.to_string().contains("Invalid cache configuration"));
        }

        #[test]
        fn test_compression_error_creation() {
            let error = CacheError::Compression {
                operation: "compress",
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid data",
                )),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error
                .to_string()
                .contains("Compression error during compress"));
            assert!(error.to_string().contains("invalid data"));
        }
    }

    mod security_error_tests {
        use super::*;

        #[test]
        fn test_signature_verification_error_creation() {
            let error = CacheError::SignatureVerification {
                algorithm: "Ed25519".to_string(),
                key_id: "key-123".to_string(),
                reason: "signature does not match".to_string(),
                recovery_hint: RecoveryHint::RegenerateKeys,
            };

            assert!(error.to_string().contains("Signature verification failed"));
            assert!(error.to_string().contains("Ed25519"));
            assert!(error.to_string().contains("key-123"));
            assert!(error.to_string().contains("signature does not match"));
        }

        #[test]
        fn test_access_denied_error_creation() {
            let error = CacheError::AccessDenied {
                operation: "read_sensitive_data".to_string(),
                required_permission: "cache:read:sensitive".to_string(),
                token_id: "token-456".to_string(),
                recovery_hint: RecoveryHint::RefreshToken,
            };

            assert!(error.to_string().contains("Access denied for operation"));
            assert!(error.to_string().contains("read_sensitive_data"));
            assert!(error.to_string().contains("cache:read:sensitive"));
            assert!(error.to_string().contains("token-456"));
        }

        #[test]
        fn test_invalid_token_error_creation() {
            let error = CacheError::InvalidToken {
                token_id: "token-789".to_string(),
                reason: TokenInvalidReason::Expired,
                recovery_hint: RecoveryHint::RefreshToken,
            };

            assert!(error.to_string().contains("Invalid token token-789"));
            assert!(error.to_string().contains("Expired"));
        }

        #[test]
        fn test_audit_log_corruption_error_creation() {
            let error = CacheError::AuditLogCorruption {
                log_file: PathBuf::from("/var/log/cache/audit.log"),
                corruption_type: AuditCorruptionType::BrokenHashChain,
                recovery_hint: RecoveryHint::ContactSecurityAdmin {
                    contact: "security@example.com".to_string(),
                },
            };

            assert!(error.to_string().contains("Audit log corruption"));
            assert!(error.to_string().contains("/var/log/cache/audit.log"));
            assert!(error.to_string().contains("BrokenHashChain"));
        }

        #[test]
        fn test_merkle_tree_corruption_error_creation() {
            let error = CacheError::MerkleTreeCorruption {
                root_hash: "root123".to_string(),
                expected_hash: "expected456".to_string(),
                corrupted_entries: vec!["entry1".to_string(), "entry2".to_string()],
                recovery_hint: RecoveryHint::RebuildMerkleTree,
            };

            assert!(error.to_string().contains("Merkle tree corruption"));
            assert!(error.to_string().contains("root hash root123"));
            assert!(error.to_string().contains("expected expected456"));
            assert!(error.to_string().contains("2 corrupted entries"));
        }

        #[test]
        fn test_rate_limit_exceeded_error_creation() {
            let error = CacheError::RateLimitExceeded {
                token_id: "rate-limited-token".to_string(),
                limit: 100.0,
                window_seconds: 60,
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_secs(60),
                },
            };

            assert!(error.to_string().contains("Rate limit exceeded"));
            assert!(error.to_string().contains("rate-limited-token"));
            assert!(error.to_string().contains("100"));
            assert!(error.to_string().contains("60 seconds"));
        }

        #[test]
        fn test_security_policy_violation_error_creation() {
            let error = CacheError::SecurityPolicyViolation {
                policy_name: "data-classification".to_string(),
                violation_details: "attempted to store classified data".to_string(),
                severity: ViolationSeverity::Critical,
                recovery_hint: RecoveryHint::ReviewSecurityPolicies,
            };

            assert!(error.to_string().contains("Security policy"));
            assert!(error.to_string().contains("data-classification"));
            assert!(error.to_string().contains("Critical"));
            assert!(error
                .to_string()
                .contains("attempted to store classified data"));
        }

        #[test]
        fn test_cryptographic_error_creation() {
            let error = CacheError::CryptographicError {
                operation: "key_derivation".to_string(),
                algorithm: "PBKDF2".to_string(),
                details: "insufficient entropy".to_string(),
                recovery_hint: RecoveryHint::RegenerateKeys,
            };

            assert!(error
                .to_string()
                .contains("Cryptographic error during key_derivation"));
            assert!(error.to_string().contains("PBKDF2"));
            assert!(error.to_string().contains("insufficient entropy"));
        }

        #[test]
        fn test_all_token_invalid_reasons() {
            let reasons = vec![
                TokenInvalidReason::Expired,
                TokenInvalidReason::Revoked,
                TokenInvalidReason::InvalidSignature,
                TokenInvalidReason::UntrustedIssuer,
                TokenInvalidReason::Malformed,
                TokenInvalidReason::NotYetValid,
                TokenInvalidReason::AudienceMismatch,
            ];

            for reason in reasons {
                let error = CacheError::InvalidToken {
                    token_id: "test-token".to_string(),
                    reason: reason.clone(),
                    recovery_hint: RecoveryHint::RefreshToken,
                };

                let error_str = error.to_string();
                assert!(error_str.contains("Invalid token test-token"));

                // Verify the reason is displayed correctly
                match reason {
                    TokenInvalidReason::Expired => assert!(error_str.contains("Expired")),
                    TokenInvalidReason::Revoked => assert!(error_str.contains("Revoked")),
                    TokenInvalidReason::InvalidSignature => {
                        assert!(error_str.contains("InvalidSignature"))
                    }
                    TokenInvalidReason::UntrustedIssuer => {
                        assert!(error_str.contains("UntrustedIssuer"))
                    }
                    TokenInvalidReason::Malformed => assert!(error_str.contains("Malformed")),
                    TokenInvalidReason::NotYetValid => assert!(error_str.contains("NotYetValid")),
                    TokenInvalidReason::AudienceMismatch => {
                        assert!(error_str.contains("AudienceMismatch"))
                    }
                }
            }
        }

        #[test]
        fn test_all_audit_corruption_types() {
            let corruption_types = vec![
                AuditCorruptionType::BrokenHashChain,
                AuditCorruptionType::InvalidEntrySignature,
                AuditCorruptionType::TimestampOutOfOrder,
                AuditCorruptionType::InvalidFormat,
                AuditCorruptionType::FileTruncated,
                AuditCorruptionType::UnauthorizedModification,
            ];

            for corruption_type in corruption_types {
                let error = CacheError::AuditLogCorruption {
                    log_file: PathBuf::from("/test/audit.log"),
                    corruption_type: corruption_type.clone(),
                    recovery_hint: RecoveryHint::ContactSecurityAdmin {
                        contact: "admin@example.com".to_string(),
                    },
                };

                let error_str = error.to_string();
                assert!(error_str.contains("Audit log corruption"));

                // Verify corruption type is displayed
                match corruption_type {
                    AuditCorruptionType::BrokenHashChain => {
                        assert!(error_str.contains("BrokenHashChain"))
                    }
                    AuditCorruptionType::InvalidEntrySignature => {
                        assert!(error_str.contains("InvalidEntrySignature"))
                    }
                    AuditCorruptionType::TimestampOutOfOrder => {
                        assert!(error_str.contains("TimestampOutOfOrder"))
                    }
                    AuditCorruptionType::InvalidFormat => {
                        assert!(error_str.contains("InvalidFormat"))
                    }
                    AuditCorruptionType::FileTruncated => {
                        assert!(error_str.contains("FileTruncated"))
                    }
                    AuditCorruptionType::UnauthorizedModification => {
                        assert!(error_str.contains("UnauthorizedModification"))
                    }
                }
            }
        }

        #[test]
        fn test_violation_severity_ordering() {
            assert!(ViolationSeverity::Low < ViolationSeverity::Medium);
            assert!(ViolationSeverity::Medium < ViolationSeverity::High);
            assert!(ViolationSeverity::High < ViolationSeverity::Critical);
        }
    }

    mod recovery_error_tests {
        use super::*;

        #[test]
        fn test_corruption_unrecoverable_error_creation() {
            let error = CacheError::CorruptionUnrecoverable {
                key: "hopeless-key".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Data is permanently lost, restore from backup".to_string(),
                },
            };

            assert!(error.to_string().contains("Corruption unrecoverable"));
            assert!(error.to_string().contains("hopeless-key"));
        }

        #[test]
        fn test_repair_in_progress_error_creation() {
            let error = CacheError::RepairInProgress {
                key: "repairing-key".to_string(),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_secs(5),
                },
            };

            assert!(error.to_string().contains("Repair in progress"));
            assert!(error.to_string().contains("repairing-key"));
        }

        #[test]
        fn test_all_repair_strategies_failed_error_creation() {
            let error = CacheError::AllRepairStrategiesFailed {
                key: "failed-repair-key".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Manual intervention required".to_string(),
                },
            };

            assert!(error.to_string().contains("All repair strategies failed"));
            assert!(error.to_string().contains("failed-repair-key"));
        }

        #[test]
        fn test_not_implemented_error_creation() {
            let error = CacheError::NotImplemented {
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Feature coming in next release".to_string(),
                },
            };

            assert!(error.to_string().contains("Feature not implemented"));
        }
    }

    mod recovery_hint_tests {
        use super::*;

        #[test]
        fn test_all_recovery_hint_types() {
            let hints = vec![
                RecoveryHint::Retry {
                    after: Duration::from_secs(1),
                },
                RecoveryHint::ClearAndRetry,
                RecoveryHint::IncreaseCapacity {
                    suggested_bytes: 1024,
                },
                RecoveryHint::CheckPermissions {
                    path: PathBuf::from("/test"),
                },
                RecoveryHint::CheckNetwork {
                    endpoint: "example.com".to_string(),
                },
                RecoveryHint::RebuildIndex,
                RecoveryHint::Manual {
                    instructions: "Test instructions".to_string(),
                },
                RecoveryHint::Ignore,
                RecoveryHint::UseDefault {
                    value: "default".to_string(),
                },
                RecoveryHint::CheckDiskSpace,
                RecoveryHint::RunEviction,
                RecoveryHint::RegenerateKeys,
                RecoveryHint::RefreshToken,
                RecoveryHint::ContactSecurityAdmin {
                    contact: "admin@example.com".to_string(),
                },
                RecoveryHint::EnableAuditLogging,
                RecoveryHint::RebuildMerkleTree,
                RecoveryHint::ReviewSecurityPolicies,
            ];

            // Test that all recovery hints can be created and formatted
            for hint in hints {
                let error = CacheError::Configuration {
                    message: "Test configuration error".to_string(),
                    recovery_hint: hint,
                };

                // All errors should be formattable
                let _ = error.to_string();

                // All errors should provide recovery hints
                let _ = error.recovery_hint();
            }
        }

        #[test]
        fn test_recovery_hint_access() {
            let error = CacheError::Corruption {
                key: "test-key".to_string(),
                reason: "test reason".to_string(),
                recovery_hint: RecoveryHint::RebuildIndex,
            };

            match error.recovery_hint() {
                RecoveryHint::RebuildIndex => {}
                _ => panic!("Expected RebuildIndex recovery hint"),
            }
        }
    }

    mod error_source_tests {
        use super::*;
        use std::error::Error;

        #[test]
        fn test_io_error_source() {
            let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
            let cache_error = CacheError::Io {
                path: PathBuf::from("/test"),
                operation: "test",
                source: io_error,
                recovery_hint: RecoveryHint::Ignore,
            };

            assert!(cache_error.source().is_some());
            assert!(cache_error
                .source()
                .unwrap()
                .to_string()
                .contains("test error"));
        }

        #[test]
        fn test_serialization_error_source() {
            // Create a real serde error by attempting to deserialize invalid JSON
            let serde_error = serde_json::from_str::<String>("invalid json").unwrap_err();

            let cache_error = CacheError::Serialization {
                key: "test-key".to_string(),
                operation: SerializationOp::Decode,
                source: Box::new(serde_error),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(cache_error.source().is_some());
        }

        #[test]
        fn test_network_error_source() {
            let network_error =
                std::io::Error::new(std::io::ErrorKind::TimedOut, "network timeout");
            let cache_error = CacheError::Network {
                endpoint: "example.com".to_string(),
                operation: "fetch",
                source: Box::new(network_error),
                recovery_hint: RecoveryHint::CheckNetwork {
                    endpoint: "example.com".to_string(),
                },
            };

            assert!(cache_error.source().is_some());
            assert!(cache_error
                .source()
                .unwrap()
                .to_string()
                .contains("network timeout"));
        }

        #[test]
        fn test_compression_error_source() {
            let compression_error =
                std::io::Error::new(std::io::ErrorKind::InvalidData, "compression failed");
            let cache_error = CacheError::Compression {
                operation: "compress",
                source: Box::new(compression_error),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(cache_error.source().is_some());
            assert!(cache_error
                .source()
                .unwrap()
                .to_string()
                .contains("compression failed"));
        }

        #[test]
        fn test_error_without_source() {
            let cache_error = CacheError::InvalidKey {
                key: "test-key".to_string(),
                reason: "invalid format".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Fix key format".to_string(),
                },
            };

            assert!(cache_error.source().is_none());
        }
    }

    mod error_conversion_tests {
        use super::*;

        #[test]
        fn test_io_error_conversion() {
            let io_error =
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
            let cache_error: CacheError = io_error.into();

            match cache_error {
                CacheError::Io {
                    operation,
                    recovery_hint,
                    ..
                } => {
                    assert_eq!(operation, "unknown");
                    match recovery_hint {
                        RecoveryHint::CheckPermissions { .. } => {}
                        _ => panic!("Expected CheckPermissions recovery hint"),
                    }
                }
                _ => panic!("Expected Io error"),
            }
        }

        #[test]
        fn test_io_error_conversion_not_found() {
            let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
            let cache_error: CacheError = io_error.into();

            match cache_error {
                CacheError::Io { recovery_hint, .. } => match recovery_hint {
                    RecoveryHint::Ignore => {}
                    _ => panic!("Expected Ignore recovery hint"),
                },
                _ => panic!("Expected Io error"),
            }
        }

        #[test]
        fn test_io_error_conversion_timeout() {
            let io_error = std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out");
            let cache_error: CacheError = io_error.into();

            match cache_error {
                CacheError::Io { recovery_hint, .. } => match recovery_hint {
                    RecoveryHint::Retry { .. } => {}
                    _ => panic!("Expected Retry recovery hint"),
                },
                _ => panic!("Expected Io error"),
            }
        }

        #[test]
        fn test_serde_json_error_conversion() {
            // Create a real serde error by attempting to deserialize invalid JSON
            let json_error = serde_json::from_str::<String>("invalid json").unwrap_err();
            let cache_error: CacheError = json_error.into();

            match cache_error {
                CacheError::Serialization {
                    operation,
                    recovery_hint,
                    ..
                } => {
                    match operation {
                        SerializationOp::Decode => {}
                        _ => panic!("Expected Decode operation"),
                    }
                    match recovery_hint {
                        RecoveryHint::Manual { .. } => {}
                        _ => panic!("Expected Manual recovery hint"),
                    }
                }
                _ => panic!("Expected Serialization error"),
            }
        }

        #[test]
        fn test_cache_error_to_core_error_conversion() {
            let cache_error = CacheError::InvalidKey {
                key: "test-key".to_string(),
                reason: "invalid format".to_string(),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Fix format".to_string(),
                },
            };

            let core_error: cuenv_core::Error = cache_error.into();
            assert!(core_error.to_string().contains("Invalid cache key"));
        }
    }

    mod error_classification_tests {
        use super::*;

        #[test]
        fn test_is_corruption_check() {
            let corruption_error = CacheError::Corruption {
                key: "test-key".to_string(),
                reason: "checksum failed".to_string(),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };
            assert!(corruption_error.is_corruption());

            let integrity_error = CacheError::IntegrityFailure {
                key: "test-key".to_string(),
                expected_hash: "abc".to_string(),
                actual_hash: "def".to_string(),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };
            assert!(integrity_error.is_corruption());

            let network_error = CacheError::Network {
                endpoint: "example.com".to_string(),
                operation: "fetch",
                source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
                recovery_hint: RecoveryHint::Retry {
                    after: Duration::from_secs(1),
                },
            };
            assert!(!network_error.is_corruption());
        }

        #[test]
        fn test_is_transient_with_network_check() {
            let network_check_error = CacheError::Network {
                endpoint: "example.com".to_string(),
                operation: "ping",
                source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
                recovery_hint: RecoveryHint::CheckNetwork {
                    endpoint: "example.com".to_string(),
                },
            };
            assert!(network_check_error.is_transient());
        }
    }

    mod store_type_tests {
        use super::*;

        #[test]
        fn test_store_type_local() {
            let error = CacheError::StoreUnavailable {
                store_type: StoreType::Local,
                reason: "disk full".to_string(),
                recovery_hint: RecoveryHint::CheckDiskSpace,
            };

            assert!(error.to_string().contains("Local"));
        }

        #[test]
        fn test_store_type_remote() {
            let error = CacheError::StoreUnavailable {
                store_type: StoreType::Remote {
                    endpoint: "remote.cache.com".to_string(),
                },
                reason: "connection failed".to_string(),
                recovery_hint: RecoveryHint::CheckNetwork {
                    endpoint: "remote.cache.com".to_string(),
                },
            };

            assert!(error.to_string().contains("Remote"));
        }

        #[test]
        fn test_store_type_content_addressed() {
            let error = CacheError::StoreUnavailable {
                store_type: StoreType::ContentAddressed,
                reason: "index corrupted".to_string(),
                recovery_hint: RecoveryHint::RebuildIndex,
            };

            assert!(error.to_string().contains("ContentAddressed"));
        }
    }

    mod serialization_op_tests {
        use super::*;

        #[test]
        fn test_serialization_op_encode() {
            // Create a real serde error by attempting to deserialize invalid JSON
            let json_error = serde_json::from_str::<String>("invalid json").unwrap_err();

            let error = CacheError::Serialization {
                key: "test-key".to_string(),
                operation: SerializationOp::Encode,
                source: Box::new(json_error),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error.to_string().contains("Encode"));
        }

        #[test]
        fn test_serialization_op_decode() {
            // Create a real serde error by attempting to deserialize invalid JSON
            let json_error = serde_json::from_str::<String>("invalid json").unwrap_err();

            let error = CacheError::Serialization {
                key: "test-key".to_string(),
                operation: SerializationOp::Decode,
                source: Box::new(json_error),
                recovery_hint: RecoveryHint::ClearAndRetry,
            };

            assert!(error.to_string().contains("Decode"));
        }
    }
}
