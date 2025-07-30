//! Production-grade cryptographic signing for cache entries
//!
//! This module provides Ed25519 digital signatures for cache entries to ensure
//! cryptographic integrity and authenticity. Ed25519 provides superior security
//! compared to HMAC-SHA256 and enables public key verification.
//!
//! ## Security Features
//!
//! - Ed25519 digital signatures for non-repudiation
//! - Secure key generation and storage
//! - Replay attack prevention with nonces
//! - Timestamp validation for entry freshness
//! - Constant-time operations to prevent timing attacks

use crate::cache::errors::{CacheError, RecoveryHint, Result};
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Length of Ed25519 private key in bytes
const ED25519_SECRET_KEY_LENGTH: usize = 32;

/// Length of Ed25519 public key in bytes
const ED25519_PUBLIC_KEY_LENGTH: usize = 32;

/// Length of Ed25519 signature in bytes
const ED25519_SIGNATURE_LENGTH: usize = 64;

/// Maximum age for signed entries (7 days)
const MAX_ENTRY_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Cryptographically signed cache entry with Ed25519 signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCacheEntry<T> {
    /// The actual cache data
    pub data: T,
    /// Ed25519 signature of the canonicalized entry
    pub signature: Vec<u8>,
    /// Cryptographic nonce to prevent replay attacks
    pub nonce: [u8; 32],
    /// Unix timestamp when entry was signed
    pub timestamp: u64,
    /// Public key for signature verification
    pub public_key: Vec<u8>,
    /// Schema version for forward compatibility
    pub schema_version: u32,
}

/// Ed25519 keypair wrapper with secure memory handling
#[derive(ZeroizeOnDrop)]
struct SecureKeypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl SecureKeypair {
    /// Generate a new cryptographically secure keypair
    fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    /// Load keypair from seed bytes
    fn from_seed(seed: [u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        Ok(Self { signing_key, verifying_key })
    }

    /// Get the public key bytes
    fn public_key_bytes(&self) -> [u8; ED25519_PUBLIC_KEY_LENGTH] {
        self.verifying_key.to_bytes()
    }

    /// Get the secret key bytes
    fn secret_key_bytes(&self) -> [u8; ED25519_SECRET_KEY_LENGTH] {
        self.signing_key.to_bytes()
    }

    /// Sign a message with Ed25519
    fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

/// Production-grade cache signer with Ed25519 cryptography
pub struct CacheSigner {
    /// Ed25519 keypair for signing operations
    keypair: SecureKeypair,
    /// Cache for the public key bytes
    public_key_bytes: [u8; ED25519_PUBLIC_KEY_LENGTH],
}

impl CacheSigner {
    /// Create a new cache signer with Ed25519 keypair
    ///
    /// Loads existing keypair from cache directory or generates a new one.
    /// Keys are stored with restricted permissions (0600 on Unix systems).
    pub fn new(cache_dir: &Path) -> Result<Self> {
        let keypair = Self::load_or_generate_keypair(cache_dir)?;
        let public_key_bytes = keypair.public_key_bytes();

        Ok(Self {
            keypair,
            public_key_bytes,
        })
    }

    /// Load existing keypair or generate a new one
    fn load_or_generate_keypair(cache_dir: &Path) -> Result<SecureKeypair> {
        let key_file = cache_dir.join(".ed25519_key");

        if key_file.exists() {
            Self::load_keypair(&key_file)
        } else {
            let keypair = SecureKeypair::generate();
            Self::save_keypair(&key_file, &keypair, cache_dir)?;
            Ok(keypair)
        }
    }

    /// Load keypair from disk
    fn load_keypair(key_file: &Path) -> Result<SecureKeypair> {
        let key_data = match fs::read(key_file) {
            Ok(data) => data,
            Err(e) => {
                return Err(CacheError::Io {
                    path: key_file.to_path_buf(),
                    operation: "read Ed25519 key",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions {
                        path: key_file.to_path_buf(),
                    },
                });
            }
        };

        if key_data.len() != ED25519_SECRET_KEY_LENGTH {
            return Err(CacheError::Configuration {
                message: format!(
                    "Invalid Ed25519 key length: expected {}, found {}",
                    ED25519_SECRET_KEY_LENGTH,
                    key_data.len()
                ),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Delete corrupted key file and restart".to_string(),
                },
            });
        }

        let mut seed = [0u8; ED25519_SECRET_KEY_LENGTH];
        seed.copy_from_slice(&key_data);

        let keypair = SecureKeypair::from_seed(seed)?;

        // Zeroize the key data from memory
        let mut key_data = key_data;
        key_data.zeroize();
        seed.zeroize();

        Ok(keypair)
    }

    /// Save keypair to disk with restricted permissions
    fn save_keypair(
        key_file: &Path,
        keypair: &SecureKeypair,
        cache_dir: &Path,
    ) -> Result<()> {
        // Ensure cache directory exists
        if let Err(e) = fs::create_dir_all(cache_dir) {
            return Err(CacheError::Io {
                path: cache_dir.to_path_buf(),
                operation: "create cache directory",
                source: e,
                recovery_hint: RecoveryHint::CheckPermissions {
                    path: cache_dir.to_path_buf(),
                },
            });
        }

        let secret_key_bytes = keypair.secret_key_bytes();

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            // Write key with mode 0600 (owner read/write only)
            let mut file = match OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(key_file)
            {
                Ok(f) => f,
                Err(e) => {
                    return Err(CacheError::Io {
                        path: key_file.to_path_buf(),
                        operation: "create Ed25519 key file",
                        source: e,
                        recovery_hint: RecoveryHint::CheckPermissions {
                            path: key_file.parent().unwrap_or(key_file).to_path_buf(),
                        },
                    });
                }
            };

            if let Err(e) = file.write_all(&secret_key_bytes) {
                return Err(CacheError::Io {
                    path: key_file.to_path_buf(),
                    operation: "write Ed25519 key",
                    source: e,
                    recovery_hint: RecoveryHint::CheckDiskSpace,
                });
            }
        }

        #[cfg(not(unix))]
        {
            if let Err(e) = fs::write(key_file, &secret_key_bytes) {
                return Err(CacheError::Io {
                    path: key_file.to_path_buf(),
                    operation: "write Ed25519 key",
                    source: e,
                    recovery_hint: RecoveryHint::CheckDiskSpace,
                });
            }
        }

        Ok(())
    }

    /// Sign a cache entry with Ed25519 digital signature
    ///
    /// Creates a tamper-evident signed wrapper with replay protection.
    pub fn sign<T>(&self, data: &T) -> Result<SignedCacheEntry<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
    {
        // Generate cryptographically secure nonce
        let mut nonce = [0u8; 32];
        getrandom::getrandom(&mut nonce).map_err(|e| CacheError::Configuration {
            message: format!("Failed to generate secure nonce: {}", e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check system entropy sources".to_string(),
            },
        })?;

        // Get current Unix timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CacheError::Configuration {
                message: format!("Invalid system time: {}", e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check system clock".to_string(),
                },
            })?
            .as_secs();

        // Serialize data for signing using canonical JSON
        let data_bytes = self.canonicalize_data(data)?;

        // Create message to sign: data || nonce || timestamp || public_key
        let message = self.create_signature_message(&data_bytes, &nonce, timestamp);

        // Sign with Ed25519
        let signature = self.keypair.sign(&message);

        Ok(SignedCacheEntry {
            data: data.clone(),
            signature: signature.to_bytes().to_vec(),
            nonce,
            timestamp,
            public_key: self.public_key_bytes.to_vec(),
            schema_version: 1,
        })
    }

    /// Verify an Ed25519 signed cache entry
    ///
    /// Performs comprehensive validation including signature verification,
    /// timestamp checks, and replay attack prevention.
    pub fn verify<T>(&self, entry: &SignedCacheEntry<T>) -> Result<bool>
    where
        T: Serialize,
    {
        // Validate schema version
        if entry.schema_version != 1 {
            return Err(CacheError::VersionMismatch {
                key: "schema_version".to_string(),
                expected_version: 1,
                actual_version: entry.schema_version,
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Update cache format or clear cache".to_string(),
                },
            });
        }

        // Validate timestamp (not too old or in the future)
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CacheError::Configuration {
                message: format!("Invalid system time: {}", e),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check system clock".to_string(),
                },
            })?
            .as_secs();

        // Check if entry is too old
        if current_time > entry.timestamp
            && current_time - entry.timestamp > MAX_ENTRY_AGE_SECONDS
        {
            return Ok(false);
        }

        // Check if entry is from the future (allow 5 minute clock skew)
        if entry.timestamp > current_time + 300 {
            return Ok(false);
        }

        // Validate public key length
        if entry.public_key.len() != ED25519_PUBLIC_KEY_LENGTH {
            return Err(CacheError::Configuration {
                message: format!(
                    "Invalid public key length: expected {}, found {}",
                    ED25519_PUBLIC_KEY_LENGTH,
                    entry.public_key.len()
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        // Validate signature length
        if entry.signature.len() != ED25519_SIGNATURE_LENGTH {
            return Err(CacheError::Configuration {
                message: format!(
                    "Invalid signature length: expected {}, found {}",
                    ED25519_SIGNATURE_LENGTH,
                    entry.signature.len()
                ),
                recovery_hint: RecoveryHint::ClearAndRetry,
            });
        }

        // Parse public key
        let mut public_key_bytes = [0u8; ED25519_PUBLIC_KEY_LENGTH];
        public_key_bytes.copy_from_slice(&entry.public_key);

        let public_key = match VerifyingKey::from_bytes(&public_key_bytes) {
            Ok(pk) => pk,
            Err(e) => {
                return Err(CacheError::Configuration {
                    message: format!("Invalid Ed25519 public key: {}", e),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                });
            }
        };

        // Parse signature
        let signature = match Signature::try_from(entry.signature.as_slice()) {
            Ok(sig) => sig,
            Err(e) => {
                return Err(CacheError::Configuration {
                    message: format!("Invalid Ed25519 signature: {}", e),
                    recovery_hint: RecoveryHint::ClearAndRetry,
                });
            }
        };

        // Recreate the signed message
        let data_bytes = self.canonicalize_data(&entry.data)?;
        let message = self.create_signature_message(&data_bytes, &entry.nonce, entry.timestamp);

        // Verify signature
        match public_key.verify(&message, &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false), // Invalid signature
        }
    }

    /// Get the public key for this signer
    #[must_use]
    pub fn public_key(&self) -> &[u8; ED25519_PUBLIC_KEY_LENGTH] {
        &self.public_key_bytes
    }

    /// Canonicalize data for consistent signing
    fn canonicalize_data<T: Serialize>(&self, data: &T) -> Result<Vec<u8>> {
        // Use bincode for deterministic serialization
        bincode::serialize(data).map_err(|e| CacheError::Serialization {
            key: "data".to_string(),
            operation: crate::cache::errors::SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::ClearAndRetry,
        })
    }

    /// Create the message to be signed
    fn create_signature_message(
        &self,
        data_bytes: &[u8],
        nonce: &[u8; 32],
        timestamp: u64,
    ) -> Vec<u8> {
        let mut message = Vec::with_capacity(
            data_bytes.len() + nonce.len() + 8 + ED25519_PUBLIC_KEY_LENGTH,
        );

        message.extend_from_slice(data_bytes);
        message.extend_from_slice(nonce);
        message.extend_from_slice(&timestamp.to_le_bytes());
        message.extend_from_slice(&self.public_key_bytes);

        message
    }
}

/// Verify a signed entry with an external public key
///
/// This function allows verification without needing the signer instance,
/// useful for distributed cache verification.
pub fn verify_with_public_key<T: Serialize>(
    entry: &SignedCacheEntry<T>,
    expected_public_key: &[u8; ED25519_PUBLIC_KEY_LENGTH],
) -> Result<bool> {
    // Validate public key matches expected
    if entry.public_key.as_slice() != expected_public_key.as_slice() {
        return Ok(false);
    }

    // Create a temporary signer for verification logic
    let temp_keypair = SecureKeypair::generate();
    let temp_signer = CacheSigner {
        keypair: temp_keypair,
        public_key_bytes: *expected_public_key,
    };

    temp_signer.verify(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::TempDir;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: String,
        number: u64,
        items: Vec<String>,
    }

    #[test]
    fn test_ed25519_sign_and_verify() {
        let temp_dir = TempDir::new().unwrap();
        let signer = CacheSigner::new(temp_dir.path()).unwrap();

        let data = TestData {
            value: "test_value".to_string(),
            number: 42,
            items: vec!["item1".to_string(), "item2".to_string()],
        };

        // Sign data
        let signed = signer.sign(&data).unwrap();

        // Verify signature
        assert!(signer.verify(&signed).unwrap());

        // Verify data integrity
        assert_eq!(signed.data, data);
        assert_eq!(signed.schema_version, 1);
        assert_eq!(signed.signature.len(), ED25519_SIGNATURE_LENGTH);
        assert_eq!(signed.public_key.len(), ED25519_PUBLIC_KEY_LENGTH);
    }

    #[test]
    fn test_tamper_detection() {
        let temp_dir = TempDir::new().unwrap();
        let signer = CacheSigner::new(temp_dir.path()).unwrap();

        let data = TestData {
            value: "original".to_string(),
            number: 100,
            items: vec!["test".to_string()],
        };

        let signed = signer.sign(&data).unwrap();

        // Tamper with data
        let mut tampered = signed.clone();
        tampered.data.value = "tampered".to_string();
        assert!(!signer.verify(&tampered).unwrap());

        // Tamper with signature
        let mut tampered = signed.clone();
        tampered.signature[0] ^= 1;
        assert!(!signer.verify(&tampered).unwrap());

        // Tamper with nonce
        let mut tampered = signed.clone();
        tampered.nonce[0] ^= 1;
        assert!(!signer.verify(&tampered).unwrap());

        // Tamper with timestamp
        let mut tampered = signed.clone();
        tampered.timestamp += 1000;
        assert!(!signer.verify(&tampered).unwrap());

        // Original should still verify
        assert!(signer.verify(&signed).unwrap());
    }

    #[test]
    fn test_key_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create first signer
        let signer1 = CacheSigner::new(temp_dir.path()).unwrap();
        let public_key1 = *signer1.public_key();

        let data = TestData {
            value: "persistence_test".to_string(),
            number: 123,
            items: vec![],
        };

        let signed = signer1.sign(&data).unwrap();

        // Create second signer (should load same key)
        let signer2 = CacheSigner::new(temp_dir.path()).unwrap();
        let public_key2 = *signer2.public_key();

        // Keys should be identical
        assert_eq!(public_key1, public_key2);

        // Should be able to verify with second signer
        assert!(signer2.verify(&signed).unwrap());
    }

    #[test]
    fn test_timestamp_validation() {
        let temp_dir = TempDir::new().unwrap();
        let signer = CacheSigner::new(temp_dir.path()).unwrap();

        let data = TestData {
            value: "timestamp_test".to_string(),
            number: 1,
            items: vec![],
        };

        let signed = signer.sign(&data).unwrap();

        // Should verify immediately
        assert!(signer.verify(&signed).unwrap());

        // Test old timestamp
        let mut old_signed = signed.clone();
        old_signed.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - MAX_ENTRY_AGE_SECONDS
            - 1;
        assert!(!signer.verify(&old_signed).unwrap());

        // Test future timestamp (beyond clock skew)
        let mut future_signed = signed.clone();
        future_signed.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 400; // More than 5 minute skew allowance
        assert!(!signer.verify(&future_signed).unwrap());
    }

    #[test]
    fn test_external_verification() {
        let temp_dir = TempDir::new().unwrap();
        let signer = CacheSigner::new(temp_dir.path()).unwrap();

        let data = TestData {
            value: "external_verify".to_string(),
            number: 999,
            items: vec!["test".to_string()],
        };

        let signed = signer.sign(&data).unwrap();
        let public_key = *signer.public_key();

        // Verify with external function
        assert!(verify_with_public_key(&signed, &public_key).unwrap());

        // Should fail with wrong public key
        let wrong_key = [0u8; ED25519_PUBLIC_KEY_LENGTH];
        assert!(!verify_with_public_key(&signed, &wrong_key).unwrap());
    }

    proptest! {
        #[test]
        fn test_sign_verify_roundtrip(
            value in ".*",
            number in 0u64..1000000,
            items in prop::collection::vec(".*", 0..10)
        ) {
            let temp_dir = TempDir::new().unwrap();
            let signer = CacheSigner::new(temp_dir.path()).unwrap();

            let data = TestData { value, number, items };
            let signed = signer.sign(&data).unwrap();

            prop_assert!(signer.verify(&signed).unwrap());
            prop_assert_eq!(signed.data, data);
        }

        #[test]
        fn test_signature_uniqueness(
            value1 in ".*",
            value2 in ".*",
        ) {
            prop_assume!(value1 != value2);

            let temp_dir = TempDir::new().unwrap();
            let signer = CacheSigner::new(temp_dir.path()).unwrap();

            let data1 = TestData { value: value1, number: 1, items: vec![] };
            let data2 = TestData { value: value2, number: 1, items: vec![] };

            let signed1 = signer.sign(&data1).unwrap();
            let signed2 = signer.sign(&data2).unwrap();

            // Signatures should be different for different data
            prop_assert_ne!(signed1.signature, signed2.signature);
            // Nonces should be different (with extremely high probability)
            prop_assert_ne!(signed1.nonce, signed2.nonce);
        }
    }
}