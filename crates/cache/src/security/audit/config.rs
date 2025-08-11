//! Configuration for audit logging

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::types::ViolationSeverity;

/// Configuration for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Log file path
    pub log_file_path: PathBuf,
    /// Maximum log file size before rotation (bytes)
    pub max_file_size: u64,
    /// Number of archived log files to keep
    pub max_archived_files: u32,
    /// Enable log compression for archived files
    pub compress_archived: bool,
    /// Flush logs to disk immediately
    pub immediate_flush: bool,
    /// Buffer size for batched writes
    pub buffer_size: usize,
    /// Include stack traces for errors
    pub include_stack_traces: bool,
    /// Log level filter
    pub min_severity: ViolationSeverity,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_file_path: PathBuf::from("cache_audit.jsonl"),
            max_file_size: 100 * 1024 * 1024, // 100MB
            max_archived_files: 10,
            compress_archived: true,
            immediate_flush: false,
            buffer_size: 8192,
            include_stack_traces: false,
            min_severity: ViolationSeverity::Low,
        }
    }
}

/// Log integrity verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogIntegrityReport {
    /// Total number of entries checked
    pub total_entries: u64,
    /// List of corrupted entry line numbers
    pub corrupted_entries: Vec<u64>,
    /// Whether the entire log passed integrity checks
    pub integrity_verified: bool,
}
