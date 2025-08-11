//! Token types and metadata structures

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache capabilities that can be granted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheCapability {
    /// Read cache entries
    Read,
    /// Write cache entries
    Write,
    /// Delete cache entries
    Delete,
    /// List cache entries
    List,
    /// Administer cache (clear, configure, etc.)
    Admin,
}

/// Cache operation permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Read cache entries
    Read,
    /// Write cache entries
    Write,
    /// Delete cache entries
    Delete,
    /// List cache keys
    List,
    /// Get cache statistics
    Statistics,
    /// Clear entire cache
    Clear,
    /// Manage other tokens (admin)
    ManageTokens,
    /// Configure cache settings
    Configure,
    /// Access audit logs
    AuditLogs,
}

/// Token metadata for additional context
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenMetadata {
    /// Human-readable description
    pub description: String,
    /// Maximum number of operations allowed
    pub max_operations: Option<u64>,
    /// Current operation count
    pub operation_count: u64,
    /// Rate limit (operations per second)
    pub rate_limit: Option<f64>,
    /// IP address restrictions (CIDR blocks)
    pub ip_restrictions: Vec<String>,
    /// Additional custom claims
    pub custom_claims: HashMap<String, String>,
}

/// Capability token for fine-grained access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Unique token identifier
    pub token_id: String,
    /// Subject (user/service) this token is issued to
    pub subject: String,
    /// Set of permissions granted by this token
    pub permissions: HashSet<Permission>,
    /// Key patterns this token has access to (glob patterns)
    pub key_patterns: Vec<String>,
    /// Token expiration timestamp (Unix milliseconds)
    pub expires_at: u64,
    /// Token issuance timestamp (Unix milliseconds)
    pub issued_at: u64,
    /// Issuer identifier
    pub issuer: String,
    /// Additional metadata
    pub metadata: TokenMetadata,
    /// Ed25519 signature of the token contents
    pub signature: Vec<u8>,
    /// Public key of the token issuer
    pub issuer_public_key: Vec<u8>,
}

impl CapabilityToken {
    /// Create a new capability token (simplified version)
    pub fn new(subject: String, capabilities: Vec<CacheCapability>, validity_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let permissions = capabilities
            .into_iter()
            .map(|cap| match cap {
                CacheCapability::Read => Permission::Read,
                CacheCapability::Write => Permission::Write,
                CacheCapability::Delete => Permission::Delete,
                CacheCapability::List => Permission::List,
                CacheCapability::Admin => Permission::ManageTokens,
            })
            .collect();

        Self {
            token_id: format!("token_{}", uuid::Uuid::new_v4()),
            subject,
            permissions,
            key_patterns: vec!["*".to_string()],
            expires_at: now + validity_seconds,
            issued_at: now,
            issuer: "system".to_string(),
            metadata: TokenMetadata::default(),
            signature: vec![],
            issuer_public_key: vec![],
        }
    }

    /// Get the token ID
    pub fn id(&self) -> &str {
        &self.token_id
    }
}
