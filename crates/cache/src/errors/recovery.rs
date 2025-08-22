//! Recovery utilities for cache errors

use super::types::{CacheError, RecoveryHint};

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
            | Self::CorruptionUnrecoverable { recovery_hint, .. }
            | Self::RepairInProgress { recovery_hint, .. }
            | Self::AllRepairStrategiesFailed { recovery_hint, .. }
            | Self::NotImplemented { recovery_hint, .. }
            | Self::SignatureVerification { recovery_hint, .. }
            | Self::AccessDenied { recovery_hint, .. }
            | Self::InvalidToken { recovery_hint, .. }
            | Self::AuditLogCorruption { recovery_hint, .. }
            | Self::MerkleTreeCorruption { recovery_hint, .. }
            | Self::RateLimitExceeded { recovery_hint, .. }
            | Self::SecurityPolicyViolation { recovery_hint, .. }
            | Self::CryptographicError { recovery_hint, .. } => recovery_hint,
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
            Self::Corruption { .. }
                | Self::IntegrityFailure { .. }
                | Self::CorruptionUnrecoverable { .. }
                | Self::AuditLogCorruption { .. }
                | Self::MerkleTreeCorruption { .. }
        )
    }
}
