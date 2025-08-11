//! Audit event types and definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{HealthStatus, SecurityViolationType, ViolationSeverity};

/// Audit event types for cache operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum AuditEvent {
    /// Cache read operation
    CacheRead {
        key: String,
        hit: bool,
        size_bytes: Option<u64>,
        duration_ms: u64,
    },
    /// Cache write operation
    CacheWrite {
        key: String,
        size_bytes: u64,
        compressed: bool,
        duration_ms: u64,
    },
    /// Cache delete operation
    CacheDelete {
        key: String,
        existed: bool,
        duration_ms: u64,
    },
    /// Cache clear operation
    CacheClear {
        entries_removed: u64,
        bytes_freed: u64,
        duration_ms: u64,
    },
    /// Cache eviction event
    CacheEviction {
        key: String,
        reason: EvictionReason,
        size_bytes: u64,
    },
    /// Authentication attempt
    Authentication {
        token_id: String,
        subject: String,
        success: bool,
        failure_reason: Option<String>,
    },
    /// Authorization check
    Authorization {
        token_id: String,
        operation: String,
        target_key: Option<String>,
        authorized: bool,
        denial_reason: Option<String>,
    },
    /// Configuration change
    ConfigurationChange {
        setting: String,
        old_value: Option<String>,
        new_value: String,
        changed_by: String,
    },
    /// Security violation detected
    SecurityViolation {
        violation_type: SecurityViolationType,
        details: String,
        severity: ViolationSeverity,
    },
    /// System health check
    HealthCheck {
        component: String,
        status: HealthStatus,
        metrics: HashMap<String, f64>,
    },
    /// Error occurrence
    Error {
        error_type: String,
        message: String,
        recoverable: bool,
        context: HashMap<String, String>,
    },
}

/// Eviction reasons for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionReason {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// Time to Live expired
    TtlExpired,
    /// Memory pressure
    MemoryPressure,
    /// Disk quota exceeded
    DiskQuota,
    /// Manual eviction
    Manual,
}
