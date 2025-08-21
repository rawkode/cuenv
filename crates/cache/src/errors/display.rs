//! Display implementations for cache errors

use super::types::CacheError;
use std::fmt;

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
            Self::CorruptionUnrecoverable { key, .. } => {
                write!(f, "Corruption unrecoverable for key: {key}")
            }
            Self::RepairInProgress { key, .. } => write!(f, "Repair in progress for key: {key}"),
            Self::AllRepairStrategiesFailed { key, .. } => {
                write!(f, "All repair strategies failed for key: {key}")
            }
            Self::NotImplemented { .. } => write!(f, "Feature not implemented"),
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
