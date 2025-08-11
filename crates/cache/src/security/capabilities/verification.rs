//! Token verification and validation logic

use crate::errors::{CacheError, RecoveryHint, Result};
use crate::security::capabilities::authority::CapabilityAuthority;
use crate::security::capabilities::tokens::CapabilityToken;
use ed25519_dalek::{Signature, Verifier};
use std::time::{SystemTime, UNIX_EPOCH};

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

impl CapabilityAuthority {
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
}

/// Public function for verifying tokens (for backward compatibility)
pub fn verify_token(
    authority: &CapabilityAuthority,
    token: &CapabilityToken,
) -> Result<TokenVerificationResult> {
    authority.verify_token(token)
}
