//! Cryptographic signing for cache entries to prevent cache poisoning attacks
//!
//! This module provides HMAC-SHA256 signing for cache entries to ensure their integrity
//! and authenticity. The signing key is derived from machine-specific data to prevent
//! cross-machine cache poisoning while allowing legitimate cache sharing.

use crate::errors::{Error, Result};
use hex;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Length of the signing key in bytes
const SIGNING_KEY_LENGTH: usize = 32;

/// Signed cache entry wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCacheEntry<T> {
    /// The actual cache data
    pub data: T,
    /// HMAC-SHA256 signature of the serialized data
    pub signature: String,
    /// Nonce to prevent replay attacks
    pub nonce: String,
    /// Timestamp for additional validation
    pub timestamp: u64,
}

/// Cache signing key manager
pub struct CacheSigner {
    /// The signing key used for HMAC operations
    signing_key: Vec<u8>,
}

impl CacheSigner {
    /// Create a new cache signer with a derived key
    pub fn new(cache_dir: &Path) -> Result<Self> {
        let key = Self::derive_signing_key(cache_dir)?;
        Ok(Self { signing_key: key })
    }

    /// Derive a signing key from machine-specific data
    fn derive_signing_key(cache_dir: &Path) -> Result<Vec<u8>> {
        // Key derivation file path
        let key_file = cache_dir.join(".signing_key");

        // Try to load existing key
        if key_file.exists() {
            let key_data = fs::read(&key_file)
                .map_err(|e| Error::file_system(&key_file, "read signing key", e))?;

            if key_data.len() != SIGNING_KEY_LENGTH {
                return Err(Error::configuration(
                    "Invalid signing key length in cache".to_string(),
                ));
            }

            return Ok(key_data);
        }

        // Generate new key if it doesn't exist
        let mut rng = rand::thread_rng();
        let mut key = vec![0u8; SIGNING_KEY_LENGTH];
        rng.fill(&mut key[..]);

        // Save key with restricted permissions
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            // Ensure cache directory exists
            fs::create_dir_all(cache_dir)
                .map_err(|e| Error::file_system(cache_dir, "create cache directory", e))?;

            // Write key with mode 0600 (owner read/write only)
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&key_file)
                .and_then(|mut f| {
                    use std::io::Write;
                    f.write_all(&key)
                })
                .map_err(|e| Error::file_system(&key_file, "write signing key", e))?;
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, just write the file normally
            fs::write(&key_file, &key)
                .map_err(|e| Error::file_system(&key_file, "write signing key", e))?;
        }

        Ok(key)
    }

    /// Sign a cache entry
    pub fn sign<T: Serialize + for<'de> Deserialize<'de> + Clone>(
        &self,
        data: &T,
    ) -> Result<SignedCacheEntry<T>> {
        // Generate nonce
        let mut rng = rand::thread_rng();
        let nonce: u64 = rng.gen();
        let nonce_str = format!("{:016x}", nonce);

        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::configuration(format!("Invalid system time: {}", e)))?
            .as_secs();

        // Serialize data for signing
        let data_json = serde_json::to_string(data).map_err(|e| Error::Json {
            message: "Failed to serialize data for signing".to_string(),
            source: e,
        })?;

        // Create signature input: data || nonce || timestamp
        let mut signature_input = Vec::new();
        signature_input.extend_from_slice(data_json.as_bytes());
        signature_input.extend_from_slice(nonce_str.as_bytes());
        signature_input.extend_from_slice(&timestamp.to_le_bytes());

        // Calculate HMAC-SHA256
        let signature = self.hmac_sha256(&signature_input);

        Ok(SignedCacheEntry {
            data: serde_json::from_str(&data_json).map_err(|e| Error::Json {
                message: "Failed to deserialize signed data".to_string(),
                source: e,
            })?,
            signature: hex::encode(signature),
            nonce: nonce_str,
            timestamp,
        })
    }

    /// Verify a signed cache entry
    pub fn verify<T: Serialize>(&self, entry: &SignedCacheEntry<T>) -> Result<bool> {
        // Check timestamp is not too old (7 days)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::configuration(format!("Invalid system time: {}", e)))?
            .as_secs();

        const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
        if current_time > entry.timestamp && current_time - entry.timestamp > MAX_AGE_SECS {
            return Ok(false);
        }

        // Serialize data for verification
        let data_json = serde_json::to_string(&entry.data).map_err(|e| Error::Json {
            message: "Failed to serialize data for verification".to_string(),
            source: e,
        })?;

        // Recreate signature input
        let mut signature_input = Vec::new();
        signature_input.extend_from_slice(data_json.as_bytes());
        signature_input.extend_from_slice(entry.nonce.as_bytes());
        signature_input.extend_from_slice(&entry.timestamp.to_le_bytes());

        // Calculate expected signature
        let expected_signature = self.hmac_sha256(&signature_input);
        let expected_hex = hex::encode(expected_signature);

        // Constant-time comparison to prevent timing attacks
        Ok(Self::constant_time_compare(&expected_hex, &entry.signature))
    }

    /// Calculate HMAC-SHA256
    fn hmac_sha256(&self, data: &[u8]) -> Vec<u8> {
        // HMAC-SHA256 implementation
        const BLOCK_SIZE: usize = 64;
        const IPAD: u8 = 0x36;
        const OPAD: u8 = 0x5C;

        // Prepare key (pad or hash if needed)
        let key = if self.signing_key.len() > BLOCK_SIZE {
            let mut hasher = Sha256::new();
            hasher.update(&self.signing_key);
            hasher.finalize().to_vec()
        } else {
            self.signing_key.clone()
        };

        // Pad key to block size
        let mut key_padded = [0u8; BLOCK_SIZE];
        key_padded[..key.len()].copy_from_slice(&key);

        // Create inner and outer padding
        let mut ipad_key = [0u8; BLOCK_SIZE];
        let mut opad_key = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            ipad_key[i] = key_padded[i] ^ IPAD;
            opad_key[i] = key_padded[i] ^ OPAD;
        }

        // Inner hash: H(K XOR ipad, data)
        let mut inner_hasher = Sha256::new();
        inner_hasher.update(ipad_key);
        inner_hasher.update(data);
        let inner_hash = inner_hasher.finalize();

        // Outer hash: H(K XOR opad, inner_hash)
        let mut outer_hasher = Sha256::new();
        outer_hasher.update(opad_key);
        outer_hasher.update(inner_hash);
        outer_hasher.finalize().to_vec()
    }

    /// Constant-time string comparison to prevent timing attacks
    fn constant_time_compare(a: &str, b: &str) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        let mut result = 0u8;

        for i in 0..a_bytes.len() {
            result |= a_bytes[i] ^ b_bytes[i];
        }

        result == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sign_and_verify() {
        let temp_dir = TempDir::new().unwrap();
        let signer = CacheSigner::new(temp_dir.path()).unwrap();

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestData {
            value: String,
            number: u32,
        }

        let data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        // Sign data
        let signed = signer.sign(&data).unwrap();

        // Verify signature
        assert!(signer.verify(&signed).unwrap());

        // Tamper with data
        let mut tampered = signed.clone();
        tampered.data.number = 43;
        assert!(!signer.verify(&tampered).unwrap());

        // Tamper with signature
        let mut tampered = signed.clone();
        tampered.signature.push('x');
        assert!(!signer.verify(&tampered).unwrap());

        // Tamper with nonce
        let mut tampered = signed.clone();
        tampered.nonce = "0000000000000000".to_string();
        assert!(!signer.verify(&tampered).unwrap());
    }

    #[test]
    fn test_key_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create first signer
        let signer1 = CacheSigner::new(temp_dir.path()).unwrap();
        let data = "test data".to_string();
        let signed = signer1.sign(&data).unwrap();

        // Create second signer (should load same key)
        let signer2 = CacheSigner::new(temp_dir.path()).unwrap();

        // Should be able to verify with second signer
        assert!(signer2.verify(&signed).unwrap());
    }
}
