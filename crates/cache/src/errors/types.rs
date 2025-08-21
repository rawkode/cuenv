//! Core error types for the cache system

use super::security::{AuditCorruptionType, TokenInvalidReason, ViolationSeverity};
use std::path::PathBuf;
use std::time::Duration;

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
}

/// Recovery hints for error handling
#[derive(Debug, Clone)]
pub enum RecoveryHint {
    /// Retry the operation
    Retry { after: Duration },

    /// Retry the operation after a delay
    RetryWithBackoff {
        initial_delay_ms: u64,
        max_retries: u32,
        backoff_multiplier: f64,
    },

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

    /// Use fallback cache
    UseFallback,

    /// Recreate cache file/directory
    Recreate,

    /// Check disk space and clean up if needed
    CheckDiskSpace,

    /// Verify cache integrity and repair if possible
    VerifyIntegrity,

    /// Update cache configuration
    UpdateConfiguration,

    /// Contact system administrator
    ContactAdmin,

    /// No recovery possible
    NoRecovery,

    /// Custom recovery action
    Custom(String),

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

/// Serialization operation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationOp {
    Encode,
    Decode,
    Serialize,
    Deserialize,
}

/// Cache store types
#[derive(Debug)]
pub enum StoreType {
    Local,
    Remote { endpoint: String },
    ContentAddressed,
    FileSystem,
    Memory,
    Redis,
    Sqlite,
    Custom(String),
}
