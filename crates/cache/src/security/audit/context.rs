//! Audit context information for tracking event sources

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Context information for audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditContext {
    /// User/service identifier
    pub principal: String,
    /// Source IP address
    pub source_ip: Option<String>,
    /// User agent or client identifier
    pub user_agent: Option<String>,
    /// Request correlation ID
    pub correlation_id: Option<String>,
    /// Session identifier
    pub session_id: Option<String>,
    /// Geographic location (country code)
    pub location: Option<String>,
    /// Additional context fields
    pub metadata: HashMap<String, String>,
}

impl Default for AuditContext {
    fn default() -> Self {
        Self {
            principal: "unknown".to_string(),
            source_ip: None,
            user_agent: None,
            correlation_id: None,
            session_id: None,
            location: None,
            metadata: HashMap::new(),
        }
    }
}
