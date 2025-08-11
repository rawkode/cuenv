//! Core types for audit logging

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::context::AuditContext;
use super::events::AuditEvent;

/// Security violation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityViolationType {
    /// Invalid signature detected
    InvalidSignature,
    /// Expired token used
    ExpiredToken,
    /// Revoked token used
    RevokedToken,
    /// Insufficient permissions
    InsufficientPermissions,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Suspicious access pattern
    SuspiciousPattern,
    /// Integrity check failed
    IntegrityFailure,
    /// Unauthorized configuration change
    UnauthorizedConfigChange,
}

/// Violation severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Health status for components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Complete audit log entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry identifier
    pub entry_id: String,
    /// Event timestamp (ISO 8601 UTC)
    pub timestamp: DateTime<Utc>,
    /// The actual audit event
    pub event: AuditEvent,
    /// Request/operation context
    pub context: AuditContext,
    /// Entry integrity hash
    pub integrity_hash: String,
    /// Previous entry hash for chain integrity
    pub previous_hash: String,
    /// Log format version
    pub schema_version: u32,
}
