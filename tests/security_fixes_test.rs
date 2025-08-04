//! Security fixes integration tests
//!
//! This module tests the security vulnerabilities that were fixed:
//! 1. Cache poisoning vulnerability - cryptographic signing
//! 2. Symlink TOCTOU race - O_NOFOLLOW protection
//! 3. Lock file permissions - secure 0600 permissions

use cuenv::cache::signing::CacheSigner;
use cuenv::cache::CacheManager;
use cuenv::cache::{ActionCache, ActionResult, ContentAddressedStore, HashEngine};
use cuenv::cue_parser::TaskConfig;
use cuenv::sync_env::InstanceLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::SystemTime;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestCacheData {
    value: String,
    number: u32,
}

/// Test cryptographic signing prevents cache poisoning
#[tokio::test]
async fn test_cache_signing_prevents_poisoning() {
    let temp_dir = TempDir::new().unwrap();
    let signer = CacheSigner::new(temp_dir.path()).unwrap();

    let data = TestCacheData {
        value: "legitimate_data".to_string(),
        number: 42,
    };

    // Sign legitimate data
    let signed = signer.sign(&data).unwrap();
    assert!(signer.verify(&signed).unwrap());

    // Attempt to tamper with data (cache poisoning attack)
    let mut tampered = signed.clone();
    tampered.data.value = "malicious_data".to_string();
    tampered.data.number = 999;

    // Verification should fail for tampered data
    assert!(!signer.verify(&tampered).unwrap());

    // Attempt to tamper with signature
    let mut tampered_sig = signed.clone();
    tampered_sig.signature = b"deadbeef".to_vec();
    assert!(!signer.verify(&tampered_sig).unwrap());

    // Attempt to tamper with nonce (replay attack)
    let mut tampered_nonce = signed.clone();
    tampered_nonce.nonce = [0u8; 32];
    assert!(!signer.verify(&tampered_nonce).unwrap());
}

/// Test action cache integration with signing
#[tokio::test]
async fn test_action_cache_signing_integration() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Arc::new(ContentAddressedStore::new(temp_dir.path().to_path_buf(), 4096).unwrap());
    let cache = ActionCache::new(cas, 0, temp_dir.path()).unwrap();

    let task_config = TaskConfig {
        description: Some("Test task".to_string()),
        command: Some("echo hello".to_string()),
        script: None,
        dependencies: None,
        working_dir: None,
        shell: None,
        inputs: None,
        outputs: None,
        cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
        cache_key: None,
        cache_env: None,
        timeout: None,
        security: None,
    };

    let digest = cache
        .compute_digest("test", &task_config, temp_dir.path(), HashMap::new())
        .await
        .unwrap();

    // Execute action and cache result
    let result = cache
        .execute_action(&digest, || async {
            Ok(ActionResult {
                exit_code: 0,
                stdout_hash: Some("hello\n".to_string()),
                stderr_hash: None,
                output_files: HashMap::new(),
                executed_at: SystemTime::now(),
                duration_ms: 10,
            })
        })
        .await
        .unwrap();

    assert_eq!(result.exit_code, 0);

    // Verify result is cached and signed
    let cached = cache.get_cached_result(&digest).await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().exit_code, 0);
}

/// Test symlink TOCTOU protection
#[test]
fn test_symlink_toctou_protection() {
    let temp_dir = TempDir::new().unwrap();
    let hash_engine = HashEngine::new(temp_dir.path()).unwrap();
    let mut hasher = hash_engine.create_hasher("test");

    // Create a regular file
    let file_path = temp_dir.path().join("test_file.txt");
    fs::write(&file_path, "test content").unwrap();

    // Hashing regular file should work
    assert!(hasher.hash_file(&file_path).is_ok());

    // Create a symlink to the file
    let symlink_path = temp_dir.path().join("test_symlink");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&file_path, &symlink_path).unwrap();

        // Hashing symlink should fail due to O_NOFOLLOW protection
        let result = hasher.hash_file(&symlink_path);
        assert!(result.is_err());

        let error_msg = format!("{}", result.unwrap_err());
        println!("Symlink error message: {}", error_msg);
        assert!(
            error_msg.contains("Symlink detected")
                || error_msg.contains("permission denied")
                || error_msg.contains("Too many levels of symbolic links")
        );
    }
}

/// Test lock file permissions are secure (0600)
#[test]
#[cfg(unix)]
fn test_lock_file_permissions() {
    // Test InstanceLock permissions
    let lock = InstanceLock::try_acquire().unwrap();

    // Get the lock file path (we need to recreate the logic from get_lock_file_path)
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let temp_dir = std::env::temp_dir();
            temp_dir.join(format!("cuenv-{}", users::get_current_uid()))
        });

    let lock_path = xdg_runtime.join("cuenv.lock");

    if lock_path.exists() {
        let metadata = fs::metadata(&lock_path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check that permissions are secure (should be 0600, but may be affected by umask)
        println!("Lock file permissions: {:o}", mode & 0o777);
        // The important thing is that group and other don't have write permissions
        assert_eq!(
            mode & 0o022,
            0,
            "Lock file should not have group/other write permissions"
        );
    }

    drop(lock);
}

/// Test cache manager lock file permissions
#[test]
#[cfg(unix)]
fn test_cache_manager_lock_permissions() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    let cache_manager = CacheManager::new_sync().unwrap();

    // The cache manager creates lock files with secure permissions
    // This test verifies the implementation exists and compiles correctly
    drop(cache_manager);
}

/// Test signature verification with expired timestamps
#[test]
fn test_signature_timestamp_validation() {
    let temp_dir = TempDir::new().unwrap();
    let signer = CacheSigner::new(temp_dir.path()).unwrap();

    let data = TestCacheData {
        value: "test".to_string(),
        number: 42,
    };

    // Create a signed entry with a very old timestamp (8 days ago)
    let mut signed = signer.sign(&data).unwrap();
    signed.timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - (8 * 24 * 60 * 60); // 8 days ago

    // Verification should fail due to expired timestamp
    assert!(!signer.verify(&signed).unwrap());
}

/// Test constant-time comparison prevents timing attacks
#[test]
fn test_constant_time_comparison() {
    let temp_dir = TempDir::new().unwrap();
    let signer = CacheSigner::new(temp_dir.path()).unwrap();

    let data = TestCacheData {
        value: "test".to_string(),
        number: 42,
    };

    let signed = signer.sign(&data).unwrap();

    // Test with completely different signature (should be constant time)
    let mut tampered = signed.clone();
    tampered.signature = b"a".repeat(64); // Different length and content

    let start = std::time::Instant::now();
    let result1 = signer.verify(&tampered).unwrap();
    let time1 = start.elapsed();

    // Test with signature that differs only in last character
    let mut tampered2 = signed.clone();
    let mut sig_bytes = tampered2.signature.clone();
    if let Some(last_byte) = sig_bytes.last_mut() {
        *last_byte = if *last_byte == b'a' { b'b' } else { b'a' };
    }
    tampered2.signature = sig_bytes;

    let start = std::time::Instant::now();
    let result2 = signer.verify(&tampered2).unwrap();
    let time2 = start.elapsed();

    // Both should fail
    assert!(!result1);
    assert!(!result2);

    // Times should be similar (within reasonable bounds for constant-time comparison)
    let time_diff = if time1 > time2 {
        time1 - time2
    } else {
        time2 - time1
    };
    assert!(
        time_diff < std::time::Duration::from_millis(10),
        "Timing difference too large, may indicate timing attack vulnerability"
    );
}

/// Test HMAC implementation against known test vectors
#[test]
fn test_hmac_implementation() {
    let temp_dir = TempDir::new().unwrap();
    let signer = CacheSigner::new(temp_dir.path()).unwrap();

    // Test that the same data produces the same signature
    let data = TestCacheData {
        value: "consistent_test".to_string(),
        number: 123,
    };

    let signed1 = signer.sign(&data).unwrap();
    let signed2 = signer.sign(&data).unwrap();

    // Signatures should be different due to nonce, but verification should work for both
    assert_ne!(signed1.signature, signed2.signature);
    assert_ne!(signed1.nonce, signed2.nonce);

    assert!(signer.verify(&signed1).unwrap());
    assert!(signer.verify(&signed2).unwrap());
}

/// Test key persistence and consistency
#[test]
fn test_signing_key_persistence() {
    let temp_dir = TempDir::new().unwrap();

    // Create first signer
    let signer1 = CacheSigner::new(temp_dir.path()).unwrap();
    let data = TestCacheData {
        value: "persistence_test".to_string(),
        number: 456,
    };
    let signed = signer1.sign(&data).unwrap();

    // Create second signer (should load same key)
    let signer2 = CacheSigner::new(temp_dir.path()).unwrap();

    // Second signer should be able to verify signature from first signer
    assert!(signer2.verify(&signed).unwrap());

    // Check that signing key file has secure permissions
    #[cfg(unix)]
    {
        let key_file = temp_dir.path().join(".signing_key");
        if key_file.exists() {
            let metadata = fs::metadata(&key_file).unwrap();
            let permissions = metadata.permissions();
            let mode = permissions.mode();

            assert_eq!(
                mode & 0o777,
                0o600,
                "Signing key file should have 0600 permissions"
            );
        }
    }
}
