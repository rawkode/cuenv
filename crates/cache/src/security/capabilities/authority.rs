//! Token authority for issuing and managing capability tokens

use crate::errors::{CacheError, RecoveryHint, Result, SerializationOp};
use crate::security::capabilities::tokens::{CapabilityToken, Permission, TokenMetadata};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Helper struct for token signing (excludes signature field)
#[derive(Serialize)]
pub struct TokenForSigning<'a> {
    pub token_id: &'a str,
    pub subject: &'a str,
    pub permissions: &'a HashSet<Permission>,
    pub key_patterns: &'a Vec<String>,
    pub expires_at: u64,
    pub issued_at: u64,
    pub issuer: &'a str,
    pub metadata: &'a TokenMetadata,
    pub issuer_public_key: &'a Vec<u8>,
}

/// Capability authority for issuing and verifying tokens
#[derive(Debug)]
pub struct CapabilityAuthority {
    /// Ed25519 signing key for signing tokens
    signing_key: SigningKey,
    /// Ed25519 verifying key
    pub(crate) verifying_key: VerifyingKey,
    /// Authority identifier
    pub(crate) authority_id: String,
    /// Set of issued tokens for revocation tracking
    pub(crate) issued_tokens: HashSet<String>,
    /// Revoked tokens (blacklist)
    pub(crate) revoked_tokens: HashSet<String>,
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
    pub(crate) fn generate_token_id(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut rng_bytes = [0u8; 32];
        getrandom::getrandom(&mut rng_bytes).expect("Failed to generate random bytes");

        let mut hasher = Sha256::new();
        hasher.update(rng_bytes);
        hasher.update(self.authority_id.as_bytes());
        hasher.update(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );

        let hash = hasher.finalize();
        hex::encode(&hash[..16]) // Use first 16 bytes for 32-char hex string
    }

    /// Serialize token for signing (excludes signature field)
    pub(crate) fn serialize_token_for_signing(&self, token: &CapabilityToken) -> Result<Vec<u8>> {
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
            operation: SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Token serialization failed".to_string(),
            },
        })
    }
}

/// Public function for issuing tokens (for backward compatibility)
pub fn issue_token(
    authority: &mut CapabilityAuthority,
    subject: String,
    permissions: HashSet<Permission>,
    key_patterns: Vec<String>,
    validity_duration: Duration,
    metadata: Option<TokenMetadata>,
) -> Result<CapabilityToken> {
    authority.issue_token(
        subject,
        permissions,
        key_patterns,
        validity_duration,
        metadata,
    )
}
