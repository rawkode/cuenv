//! Security-related error types for the cache system

/// Token invalid reasons
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenInvalidReason {
    /// Token has expired
    Expired,
    /// Token signature is invalid
    InvalidSignature,
    /// Token format is malformed
    Malformed,
    /// Token has been revoked
    Revoked,
    /// Token scope is insufficient
    InsufficientScope,
    /// Token issuer is not trusted
    UntrustedIssuer,
    /// Token audience mismatch
    AudienceMismatch,
    /// Token replay attack detected
    ReplayAttack,
    /// Token algorithm not supported
    UnsupportedAlgorithm,
    /// Token key not found
    KeyNotFound,
    /// Token claim missing
    MissingClaim(String),
    /// Token claim invalid
    InvalidClaim(String, String),
    /// Custom reason
    Custom(String),
}

/// Audit corruption types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditCorruptionType {
    /// Missing log entries
    MissingEntries,
    /// Invalid checksums
    InvalidChecksums,
    /// Sequence gaps detected
    SequenceGaps,
    /// Timestamp inconsistencies
    TimestampInconsistency,
    /// Hash chain broken
    BrokenHashChain,
    /// Digital signature invalid
    InvalidSignature,
    /// Log file truncated
    FileTruncated,
    /// Unauthorized modifications
    UnauthorizedModifications,
    /// Encryption errors
    EncryptionCorruption,
    /// Schema version mismatch
    SchemaMismatch,
    /// Custom corruption type
    Custom(String),
}

/// Violation severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViolationSeverity {
    /// Informational
    Info,
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}
