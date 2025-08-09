//! Capability-based access control for cache operations
//!
//! This module implements a fine-grained access control system using capability tokens.
//! Each operation requires appropriate capabilities, preventing unauthorized access
//! and enabling secure multi-tenant cache usage.
//!
//! ## Security Model
//!
//! - Capabilities are cryptographically signed tokens
//! - Each token grants specific permissions (read, write, admin)
//! - Tokens include expiration and scope constraints
//! - Zero-trust model: all operations must be authorized

use crate::errors::{CacheError, RecoveryHint, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// Token metadata for additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub custom_claims: std::collections::HashMap<String, String>,
}

impl Default for TokenMetadata {
    fn default() -> Self {
        Self {
            description: String::new(),
            max_operations: None,
            operation_count: 0,
            rate_limit: None,
            ip_restrictions: Vec::new(),
            custom_claims: std::collections::HashMap::new(),
        }
    }
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

/// Capability authority for issuing and verifying tokens
#[derive(Debug)]
pub struct CapabilityAuthority {
    /// Ed25519 signing key for signing tokens
    signing_key: SigningKey,
    /// Ed25519 verifying key
    verifying_key: VerifyingKey,
    /// Authority identifier
    authority_id: String,
    /// Set of issued tokens for revocation tracking
    issued_tokens: HashSet<String>,
    /// Revoked tokens (blacklist)
    revoked_tokens: HashSet<String>,
}

// Manual implementation of Drop for secure cleanup
impl Drop for CapabilityAuthority {
    fn drop(&mut self) {
        // ed25519_dalek's SigningKey doesn't implement Zeroize
        // The signing key will be cleared when dropped
        // Clear sensitive data from collections
        self.issued_tokens.clear();
        self.revoked_tokens.clear();
    }
}

impl CapabilityAuthority {
    /// Create a new capability authority
    pub fn new(authority_id: String) -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key,
            verifying_key,
            authority_id,
            issued_tokens: HashSet::new(),
            revoked_tokens: HashSet::new(),
        }
    }

    /// Load authority from existing signing key
    pub fn from_signing_key(authority_id: String, signing_key: SigningKey) -> Self {
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
            authority_id,
            issued_tokens: HashSet::new(),
            revoked_tokens: HashSet::new(),
        }
    }

    /// Issue a new capability token
    pub fn issue_token(
        &mut self,
        subject: String,
        permissions: HashSet<Permission>,
        key_patterns: Vec<String>,
        validity_duration: Duration,
        metadata: Option<TokenMetadata>,
    ) -> Result<CapabilityToken> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CacheError::Configuration {
                message: format!("Invalid system time: {e}"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check system clock".to_string(),
                },
            })?
            .as_secs();

        let expires_at = now + validity_duration.as_secs();
        let token_id = self.generate_token_id();

        let mut token = CapabilityToken {
            token_id: token_id.clone(),
            subject,
            permissions,
            key_patterns,
            expires_at,
            issued_at: now,
            issuer: self.authority_id.clone(),
            metadata: metadata.unwrap_or_default(),
            signature: Vec::new(), // Will be set after signing
            issuer_public_key: self.verifying_key.to_bytes().to_vec(),
        };

        // Sign the token
        let token_bytes = self.serialize_token_for_signing(&token)?;
        let signature = self.signing_key.sign(&token_bytes);
        token.signature = signature.to_bytes().to_vec();

        // Track issued token
        self.issued_tokens.insert(token_id);

        Ok(token)
    }

    /// Verify a capability token
    pub fn verify_token(&self, token: &CapabilityToken) -> Result<TokenVerificationResult> {
        // Check if token is revoked
        if self.revoked_tokens.contains(&token.token_id) {
            return Ok(TokenVerificationResult::Revoked);
        }

        // Check expiration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CacheError::Configuration {
                message: format!("Invalid system time: {e}"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check system clock".to_string(),
                },
            })?
            .as_secs();

        if now > token.expires_at {
            return Ok(TokenVerificationResult::Expired);
        }

        // Verify issuer
        if token.issuer != self.authority_id {
            return Ok(TokenVerificationResult::InvalidIssuer);
        }

        // Verify public key
        let expected_public_key = self.verifying_key.to_bytes();
        if token.issuer_public_key != expected_public_key {
            return Ok(TokenVerificationResult::InvalidPublicKey);
        }

        // Verify signature
        let token_bytes = self.serialize_token_for_signing(token)?;

        let signature = match Signature::try_from(token.signature.as_slice()) {
            Ok(sig) => sig,
            Err(e) => {
                return Err(CacheError::Configuration {
                    message: format!("Invalid signature format: {e}"),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                });
            }
        };

        match self.verifying_key.verify(&token_bytes, &signature) {
            Ok(()) => Ok(TokenVerificationResult::Valid),
            Err(_) => Ok(TokenVerificationResult::InvalidSignature),
        }
    }

    /// Revoke a capability token
    pub fn revoke_token(&mut self, token_id: &str) -> Result<bool> {
        if self.issued_tokens.contains(token_id) {
            self.revoked_tokens.insert(token_id.to_string());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get authority public key
    #[must_use]
    pub fn public_key(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Get authority ID
    #[must_use]
    pub fn authority_id(&self) -> &str {
        &self.authority_id
    }

    /// Generate a cryptographically secure token ID
    fn generate_token_id(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut rng_bytes = [0u8; 32];
        getrandom::getrandom(&mut rng_bytes).expect("Failed to generate random bytes");

        let mut hasher = Sha256::new();
        hasher.update(&rng_bytes);
        hasher.update(self.authority_id.as_bytes());
        hasher.update(
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );

        let hash = hasher.finalize();
        hex::encode(&hash[..16]) // Use first 16 bytes for 32-char hex string
    }

    /// Serialize token for signing (excludes signature field)
    fn serialize_token_for_signing(&self, token: &CapabilityToken) -> Result<Vec<u8>> {
        let signing_token = TokenForSigning {
            token_id: &token.token_id,
            subject: &token.subject,
            permissions: &token.permissions,
            key_patterns: &token.key_patterns,
            expires_at: token.expires_at,
            issued_at: token.issued_at,
            issuer: &token.issuer,
            metadata: &token.metadata,
            issuer_public_key: &token.issuer_public_key,
        };

        bincode::serialize(&signing_token).map_err(|e| CacheError::Serialization {
            key: "capability_token".to_string(),
            operation: crate::errors::SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Token serialization failed".to_string(),
            },
        })
    }
}

/// Token verification result
#[derive(Debug, Clone, PartialEq)]
pub enum TokenVerificationResult {
    /// Token is valid and not expired
    Valid,
    /// Token has expired
    Expired,
    /// Token has been revoked
    Revoked,
    /// Token signature is invalid
    InvalidSignature,
    /// Token issuer is not recognized
    InvalidIssuer,
    /// Token public key doesn't match issuer
    InvalidPublicKey,
}

/// Capability checker for authorizing cache operations
#[derive(Debug)]
pub struct CapabilityChecker {
    /// Authority for token verification
    authority: CapabilityAuthority,
    /// Rate limiting state
    rate_limits: std::collections::HashMap<String, RateLimitState>,
}

impl CapabilityChecker {
    /// Create a new capability checker
    pub fn new(authority: CapabilityAuthority) -> Self {
        Self {
            authority,
            rate_limits: std::collections::HashMap::new(),
        }
    }

    /// Issue a new capability token
    pub fn issue_token(
        &mut self,
        subject: String,
        permissions: HashSet<Permission>,
        key_patterns: Vec<String>,
        validity_duration: Duration,
        metadata: Option<TokenMetadata>,
    ) -> Result<CapabilityToken> {
        self.authority.issue_token(
            subject,
            permissions,
            key_patterns,
            validity_duration,
            metadata,
        )
    }

    /// Check if a token has permission for a specific operation
    pub fn check_permission(
        &mut self,
        token: &CapabilityToken,
        operation: &CacheOperation,
    ) -> Result<AuthorizationResult> {
        // Verify token validity
        match self.authority.verify_token(token)? {
            TokenVerificationResult::Valid => {}
            result => return Ok(AuthorizationResult::TokenInvalid(result)),
        }

        // Check permission for operation
        let required_permission = operation.required_permission();
        if !token.permissions.contains(&required_permission) {
            return Ok(AuthorizationResult::InsufficientPermissions);
        }

        // Check key pattern access
        if !self.check_key_access(token, operation) {
            return Ok(AuthorizationResult::KeyAccessDenied);
        }

        // Check rate limits
        if let Some(rate_limit) = token.metadata.rate_limit {
            if !self.check_rate_limit(&token.token_id, rate_limit) {
                return Ok(AuthorizationResult::RateLimitExceeded);
            }
        }

        // Check operation count limits
        if let Some(max_ops) = token.metadata.max_operations {
            if token.metadata.operation_count >= max_ops {
                return Ok(AuthorizationResult::OperationLimitExceeded);
            }
        }

        Ok(AuthorizationResult::Authorized)
    }

    /// Check if token has access to a specific key
    fn check_key_access(&self, token: &CapabilityToken, operation: &CacheOperation) -> bool {
        let key = match operation.target_key() {
            Some(k) => k,
            None => return true, // Operations without specific keys are allowed
        };

        // Check against key patterns
        for pattern in &token.key_patterns {
            if self.matches_pattern(key, pattern) {
                return true;
            }
        }

        false
    }

    /// Check pattern matching for key access
    fn matches_pattern(&self, key: &str, pattern: &str) -> bool {
        // Simple glob pattern matching
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return key.starts_with(prefix);
        }

        if let Some(suffix) = pattern.strip_prefix('*') {
            return key.ends_with(suffix);
        }

        key == pattern
    }

    /// Check rate limiting
    fn check_rate_limit(&mut self, token_id: &str, rate_limit: f64) -> bool {
        let now = SystemTime::now();
        let rate_state = self
            .rate_limits
            .entry(token_id.to_string())
            .or_insert_with(|| RateLimitState {
                last_operation: now,
                operation_count: 0,
                window_start: now,
            });

        let window_duration = Duration::from_secs(1); // 1-second window
        let max_operations = rate_limit as u64;

        // Reset window if needed
        if now
            .duration_since(rate_state.window_start)
            .unwrap_or_default()
            >= window_duration
        {
            rate_state.window_start = now;
            rate_state.operation_count = 0;
        }

        // Check if under limit
        if rate_state.operation_count < max_operations {
            rate_state.operation_count += 1;
            rate_state.last_operation = now;
            true
        } else {
            false
        }
    }
}

/// Rate limiting state
#[derive(Debug)]
struct RateLimitState {
    last_operation: SystemTime,
    operation_count: u64,
    window_start: SystemTime,
}

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

/// Authorization result
#[derive(Debug, Clone, PartialEq)]
pub enum AuthorizationResult {
    /// Operation is authorized
    Authorized,
    /// Token is invalid
    TokenInvalid(TokenVerificationResult),
    /// Insufficient permissions for operation
    InsufficientPermissions,
    /// Key access denied by patterns
    KeyAccessDenied,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Operation count limit exceeded
    OperationLimitExceeded,
}

/// Helper struct for token signing (excludes signature field)
#[derive(Serialize)]
struct TokenForSigning<'a> {
    token_id: &'a str,
    subject: &'a str,
    permissions: &'a HashSet<Permission>,
    key_patterns: &'a Vec<String>,
    expires_at: u64,
    issued_at: u64,
    issuer: &'a str,
    metadata: &'a TokenMetadata,
    issuer_public_key: &'a Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_token_issue_and_verify() {
        let mut authority = CapabilityAuthority::new("test-authority".to_string());

        let mut permissions = HashSet::new();
        permissions.insert(Permission::Read);
        permissions.insert(Permission::Write);

        let token = authority
            .issue_token(
                "test-user".to_string(),
                permissions,
                vec!["test/*".to_string()],
                Duration::from_secs(3600),
                None,
            )
            .unwrap();

        let result = authority.verify_token(&token).unwrap();
        assert_eq!(result, TokenVerificationResult::Valid);
    }

    #[test]
    fn test_token_expiration() {
        let mut authority = CapabilityAuthority::new("test-authority".to_string());

        let token = authority
            .issue_token(
                "test-user".to_string(),
                [Permission::Read].into_iter().collect(),
                vec!["*".to_string()],
                Duration::from_secs(1), // Use seconds for reliable expiration
                None,
            )
            .unwrap();

        // Wait for expiration
        thread::sleep(Duration::from_secs(2));

        let result = authority.verify_token(&token).unwrap();
        assert_eq!(result, TokenVerificationResult::Expired);
    }

    #[test]
    fn test_token_revocation() {
        let mut authority = CapabilityAuthority::new("test-authority".to_string());

        let token = authority
            .issue_token(
                "test-user".to_string(),
                [Permission::Read].into_iter().collect(),
                vec!["*".to_string()],
                Duration::from_secs(3600),
                None,
            )
            .unwrap();

        // Verify initially valid
        assert_eq!(
            authority.verify_token(&token).unwrap(),
            TokenVerificationResult::Valid
        );

        // Revoke token
        assert!(authority.revoke_token(&token.token_id).unwrap());

        // Should now be revoked
        assert_eq!(
            authority.verify_token(&token).unwrap(),
            TokenVerificationResult::Revoked
        );
    }

    #[test]
    fn test_capability_checking() {
        let authority = CapabilityAuthority::new("test-authority".to_string());
        let mut checker = CapabilityChecker::new(authority);

        let token = checker
            .authority
            .issue_token(
                "test-user".to_string(),
                [Permission::Read, Permission::Write].into_iter().collect(),
                vec!["cache/*".to_string()],
                Duration::from_secs(3600),
                None,
            )
            .unwrap();

        // Should allow read operation on allowed key
        let read_op = CacheOperation::Read {
            key: "cache/test".to_string(),
        };
        let result = checker.check_permission(&token, &read_op).unwrap();
        assert_eq!(result, AuthorizationResult::Authorized);

        // Should deny read operation on disallowed key
        let read_op = CacheOperation::Read {
            key: "other/test".to_string(),
        };
        let result = checker.check_permission(&token, &read_op).unwrap();
        assert_eq!(result, AuthorizationResult::KeyAccessDenied);

        // Should deny operation without permission
        let clear_op = CacheOperation::Clear;
        let result = checker.check_permission(&token, &clear_op).unwrap();
        assert_eq!(result, AuthorizationResult::InsufficientPermissions);
    }

    #[test]
    fn test_pattern_matching() {
        let authority = CapabilityAuthority::new("test-authority".to_string());
        let checker = CapabilityChecker::new(authority);

        // Test wildcard patterns
        assert!(checker.matches_pattern("any/key", "*"));
        assert!(checker.matches_pattern("prefix/test", "prefix/*"));
        assert!(checker.matches_pattern("test/suffix", "*/suffix"));
        assert!(checker.matches_pattern("exact", "exact"));

        // Test non-matches
        assert!(!checker.matches_pattern("other/key", "prefix/*"));
        assert!(!checker.matches_pattern("prefix/test", "*/suffix"));
        assert!(!checker.matches_pattern("almost", "exact"));
    }

    #[test]
    fn test_rate_limiting() {
        let authority = CapabilityAuthority::new("test-authority".to_string());
        let mut checker = CapabilityChecker::new(authority);

        let mut metadata = TokenMetadata::default();
        metadata.rate_limit = Some(2.0); // 2 operations per second

        let token = checker
            .authority
            .issue_token(
                "test-user".to_string(),
                [Permission::Read].into_iter().collect(),
                vec!["*".to_string()],
                Duration::from_secs(3600),
                Some(metadata),
            )
            .unwrap();

        let read_op = CacheOperation::Read {
            key: "test".to_string(),
        };

        // First two operations should succeed
        assert_eq!(
            checker.check_permission(&token, &read_op).unwrap(),
            AuthorizationResult::Authorized
        );
        assert_eq!(
            checker.check_permission(&token, &read_op).unwrap(),
            AuthorizationResult::Authorized
        );

        // Third operation should be rate limited
        assert_eq!(
            checker.check_permission(&token, &read_op).unwrap(),
            AuthorizationResult::RateLimitExceeded
        );
    }
}
