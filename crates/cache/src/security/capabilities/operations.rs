//! Cache operation types and permission mappings

use crate::security::capabilities::tokens::Permission;

/// Cache operation types for authorization
#[derive(Debug, Clone)]
pub enum CacheOperation {
    Read { key: String },
    Write { key: String },
    Delete { key: String },
    List { pattern: Option<String> },
    Statistics,
    Clear,
    Configure,
    AuditLog,
}

impl CacheOperation {
    /// Get the required permission for this operation
    #[must_use]
    pub const fn required_permission(&self) -> Permission {
        match self {
            Self::Read { .. } => Permission::Read,
            Self::Write { .. } => Permission::Write,
            Self::Delete { .. } => Permission::Delete,
            Self::List { .. } => Permission::List,
            Self::Statistics => Permission::Statistics,
            Self::Clear => Permission::Clear,
            Self::Configure => Permission::Configure,
            Self::AuditLog => Permission::AuditLogs,
        }
    }

    /// Get the target key for this operation (if applicable)
    #[must_use]
    pub fn target_key(&self) -> Option<&str> {
        match self {
            Self::Read { key } | Self::Write { key } | Self::Delete { key } => Some(key),
            _ => None,
        }
    }
}
