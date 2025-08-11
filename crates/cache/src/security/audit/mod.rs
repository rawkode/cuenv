//! Production-grade audit logging for cache operations
//!
//! This module provides comprehensive audit logging with tamper-evident logs,
//! structured event recording, and compliance-ready output formats.
//!
//! ## Security Features
//!
//! - Append-only log files with integrity protection
//! - Structured logging with JSON output
//! - Automatic log rotation and archival
//! - Tamper detection using cryptographic hashes
//! - Performance optimized with async I/O

mod config;
mod context;
mod events;
mod integrity;
mod logger;
mod types;

pub use config::{AuditConfig, LogIntegrityReport};
pub use context::AuditContext;
pub use events::{AuditEvent, EvictionReason};
pub use logger::AuditLogger;
pub use types::{AuditLogEntry, HealthStatus, SecurityViolationType, ViolationSeverity};
