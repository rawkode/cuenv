//! Authorization and permission checking logic

use crate::errors::Result;
use crate::security::capabilities::{
    authority::CapabilityAuthority,
    limiting::RateLimiter,
    operations::CacheOperation,
    patterns::matches_pattern,
    tokens::{CapabilityToken, Permission, TokenMetadata},
    verification::TokenVerificationResult,
};
use std::collections::HashSet;
use std::time::Duration;

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

/// Capability checker for authorizing cache operations
#[derive(Debug)]
pub struct CapabilityChecker {
    /// Authority for token verification
    authority: CapabilityAuthority,
    /// Rate limiter
    rate_limiter: RateLimiter,
}

impl CapabilityChecker {
    /// Create a new capability checker
    pub fn new(authority: CapabilityAuthority) -> Self {
        Self {
            authority,
            rate_limiter: RateLimiter::new(),
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
        if !check_key_access(token, operation) {
            return Ok(AuthorizationResult::KeyAccessDenied);
        }

        // Check rate limits
        if let Some(rate_limit) = token.metadata.rate_limit {
            if !self
                .rate_limiter
                .check_rate_limit(&token.token_id, rate_limit)
            {
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

    /// Revoke a token
    pub fn revoke_token(&mut self, token_id: &str) -> Result<bool> {
        self.rate_limiter.clear_token_state(token_id);
        self.authority.revoke_token(token_id)
    }

    /// Get the authority's public key
    pub fn public_key(&self) -> [u8; 32] {
        self.authority.public_key()
    }

    /// Get the authority ID
    pub fn authority_id(&self) -> &str {
        self.authority.authority_id()
    }
}

/// Check if token has access to a specific key
fn check_key_access(token: &CapabilityToken, operation: &CacheOperation) -> bool {
    let key = match operation.target_key() {
        Some(k) => k,
        None => return true, // Operations without specific keys are allowed
    };

    // Check against key patterns
    for pattern in &token.key_patterns {
        if matches_pattern(key, pattern) {
            return true;
        }
    }

    false
}

/// Public function for checking permissions (for backward compatibility)
pub fn check_permission(
    checker: &mut CapabilityChecker,
    token: &CapabilityToken,
    operation: &CacheOperation,
) -> Result<AuthorizationResult> {
    checker.check_permission(token, operation)
}
