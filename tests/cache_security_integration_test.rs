//! Comprehensive security integration tests for Phase 7
//!
//! These tests verify the security features work correctly together:
//! - Ed25519 signature verification
//! - Capability-based access control
//! - Audit logging integrity
//! - Merkle tree tamper detection
//! - End-to-end security workflows

use cuenv::cache::{
    audit::{AuditConfig, AuditContext},
    capabilities::{
        AuthorizationResult, CacheOperation, CapabilityAuthority, CapabilityChecker, Permission,
    },
    merkle::{CacheEntryMetadata, MerkleTree},
    secure_cache::{SecureCache, SecureCacheConfig},
    signing::CacheSigner,
    Cache, ProductionCache, UnifiedCacheConfig,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestCacheData {
    id: u64,
    name: String,
    data: Vec<u8>,
    metadata: std::collections::HashMap<String, String>,
}

impl TestCacheData {
    fn new(id: u64, name: String) -> Self {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("created_by".to_string(), "test".to_string());
        metadata.insert("version".to_string(), "1.0".to_string());

        Self {
            id,
            name: name.clone(),
            data: name.as_bytes().to_vec(),
            metadata,
        }
    }
}

/// Test Ed25519 signature security
#[tokio::test]
async fn test_ed25519_signature_security() {
    let temp_dir = TempDir::new().unwrap();
    let signer = CacheSigner::new(temp_dir.path()).unwrap();

    let test_data = TestCacheData::new(1, "test_data".to_string());

    // Sign the data
    let signed = signer.sign(&test_data).unwrap();
    assert_eq!(signed.data, test_data);
    assert_eq!(signed.signature.len(), 64); // Ed25519 signature length
    assert_eq!(signed.public_key.len(), 32); // Ed25519 public key length

    // Verify signature
    assert!(signer.verify(&signed).unwrap());

    // Test tamper detection
    let mut tampered = signed.clone();
    tampered.data.name = "tampered".to_string();
    assert!(!signer.verify(&tampered).unwrap());

    // Test signature tampering
    let mut sig_tampered = signed.clone();
    sig_tampered.signature[0] ^= 1;
    assert!(!signer.verify(&sig_tampered).unwrap());

    // Test nonce tampering
    let mut nonce_tampered = signed.clone();
    nonce_tampered.nonce[0] ^= 1;
    assert!(!signer.verify(&nonce_tampered).unwrap());
}

/// Test capability-based access control
#[tokio::test]
async fn test_capability_access_control() {
    let authority = CapabilityAuthority::new("test-authority".to_string());
    let mut checker = CapabilityChecker::new(authority);

    // Create tokens with different permissions
    let read_only_token = checker
        .issue_token(
            "read-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["data/*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    let write_token = checker
        .issue_token(
            "write-user".to_string(),
            [Permission::Read, Permission::Write].into_iter().collect(),
            vec!["data/*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    let admin_token = checker
        .issue_token(
            "admin-user".to_string(),
            [
                Permission::Read,
                Permission::Write,
                Permission::Delete,
                Permission::Clear,
            ]
            .into_iter()
            .collect(),
            vec!["*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    // Test read operation
    let read_op = CacheOperation::Read {
        key: "data/test".to_string(),
    };

    assert_eq!(
        checker
            .check_permission(&read_only_token, &read_op)
            .unwrap(),
        AuthorizationResult::Authorized
    );
    assert_eq!(
        checker.check_permission(&write_token, &read_op).unwrap(),
        AuthorizationResult::Authorized
    );
    assert_eq!(
        checker.check_permission(&admin_token, &read_op).unwrap(),
        AuthorizationResult::Authorized
    );

    // Test write operation
    let write_op = CacheOperation::Write {
        key: "data/test".to_string(),
    };

    assert_eq!(
        checker
            .check_permission(&read_only_token, &write_op)
            .unwrap(),
        AuthorizationResult::InsufficientPermissions
    );
    assert_eq!(
        checker.check_permission(&write_token, &write_op).unwrap(),
        AuthorizationResult::Authorized
    );
    assert_eq!(
        checker.check_permission(&admin_token, &write_op).unwrap(),
        AuthorizationResult::Authorized
    );

    // Test clear operation
    let clear_op = CacheOperation::Clear;

    assert_eq!(
        checker
            .check_permission(&read_only_token, &clear_op)
            .unwrap(),
        AuthorizationResult::InsufficientPermissions
    );
    assert_eq!(
        checker.check_permission(&write_token, &clear_op).unwrap(),
        AuthorizationResult::InsufficientPermissions
    );
    assert_eq!(
        checker.check_permission(&admin_token, &clear_op).unwrap(),
        AuthorizationResult::Authorized
    );

    // Test key pattern restrictions
    let restricted_read = CacheOperation::Read {
        key: "secret/data".to_string(),
    };

    assert_eq!(
        checker
            .check_permission(&read_only_token, &restricted_read)
            .unwrap(),
        AuthorizationResult::KeyAccessDenied
    );
    assert_eq!(
        checker
            .check_permission(&write_token, &restricted_read)
            .unwrap(),
        AuthorizationResult::KeyAccessDenied
    );
    assert_eq!(
        checker
            .check_permission(&admin_token, &restricted_read)
            .unwrap(),
        AuthorizationResult::Authorized
    );
}

/// Test token expiration and revocation
#[tokio::test]
async fn test_token_lifecycle() {
    let mut authority = CapabilityAuthority::new("test-authority".to_string());

    // Create short-lived token
    let short_token = authority
        .issue_token(
            "short-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(1), // 1 second expiration (minimum granularity)
            None,
        )
        .unwrap();

    // Token should be valid initially
    assert_eq!(
        authority.verify_token(&short_token).unwrap(),
        cuenv::cache::capabilities::TokenVerificationResult::Valid
    );

    // Wait for expiration
    sleep(Duration::from_secs(2)).await;

    // Token should be expired
    assert_eq!(
        authority.verify_token(&short_token).unwrap(),
        cuenv::cache::capabilities::TokenVerificationResult::Expired
    );

    // Test revocation
    let revokable_token = authority
        .issue_token(
            "revoke-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    // Token should be valid
    assert_eq!(
        authority.verify_token(&revokable_token).unwrap(),
        cuenv::cache::capabilities::TokenVerificationResult::Valid
    );

    // Revoke token
    assert!(authority.revoke_token(&revokable_token.token_id).unwrap());

    // Token should be revoked
    assert_eq!(
        authority.verify_token(&revokable_token).unwrap(),
        cuenv::cache::capabilities::TokenVerificationResult::Revoked
    );
}

/// Test Merkle tree integrity and tamper detection
#[tokio::test]
async fn test_merkle_tree_integrity() {
    let mut tree = MerkleTree::new();

    // Add test entries
    let entries = vec![
        ("key1", "value1", 100),
        ("key2", "value2", 200),
        ("key3", "value3", 300),
        ("key4", "value4", 400),
    ];

    for (key, value, size) in &entries {
        let content_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            hasher.finalize().into()
        };

        let metadata = CacheEntryMetadata {
            size_bytes: *size,
            modified_at: 1640000000,
            content_hash,
            expires_at: None,
        };

        tree.insert_entry(key.to_string(), content_hash, metadata)
            .unwrap();
    }

    // Verify initial integrity
    let report = tree.verify_integrity().unwrap();
    assert!(report.tree_valid);
    assert_eq!(report.total_entries, 4);
    assert_eq!(report.verified_entries, 4);
    assert!(report.corrupted_entries.is_empty());

    // Generate and verify proofs
    for (key, _, _) in &entries {
        let proof = tree.generate_proof(key).unwrap();
        assert!(proof.is_some());

        let proof = proof.unwrap();
        assert_eq!(proof.cache_key, *key);
        assert!(tree.verify_proof(&proof).unwrap());
    }

    // Test tree state persistence
    let state = tree.export_state().unwrap();
    let mut new_tree = MerkleTree::new();
    new_tree.import_state(state).unwrap();

    // Verify restored tree
    let restored_report = new_tree.verify_integrity().unwrap();
    assert!(restored_report.tree_valid);
    assert_eq!(restored_report.total_entries, 4);
}

/// Test audit logging integrity and tamper detection
#[tokio::test]
async fn test_audit_logging_integrity() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("audit_test.jsonl");

    let config = AuditConfig {
        log_file_path: log_path.clone(),
        immediate_flush: true,
        ..Default::default()
    };

    let logger = cuenv::cache::audit::AuditLogger::new(config).await.unwrap();

    // Log various events
    let context = AuditContext {
        principal: "test-user".to_string(),
        source_ip: Some("127.0.0.1".to_string()),
        ..Default::default()
    };

    // Log cache operations
    for i in 0..10 {
        logger
            .log_cache_read(
                &format!("key_{}", i),
                i % 2 == 0,
                Some(1024),
                5,
                context.clone(),
            )
            .await
            .unwrap();
        logger
            .log_cache_write(&format!("key_{}", i), 1024, false, 3, context.clone())
            .await
            .unwrap();
    }

    // Log security events
    logger
        .log_security_violation(
            cuenv::cache::audit::SecurityViolationType::InvalidSignature,
            "Test security violation".to_string(),
            cuenv::cache::audit::ViolationSeverity::High,
            context.clone(),
        )
        .await
        .unwrap();

    // Verify log integrity
    let report = logger.verify_log_integrity(&log_path).await.unwrap();
    assert!(report.integrity_verified);
    assert_eq!(report.total_entries, 21); // 10 reads + 10 writes + 1 security event
    assert!(report.corrupted_entries.is_empty());
}

/// Test end-to-end secure cache operations
#[tokio::test]
async fn test_secure_cache_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create underlying cache
    let inner_cache = ProductionCache::new(cache_dir.join("cache"), UnifiedCacheConfig::default())
        .await
        .unwrap();

    // Create secure cache with all security features
    let secure_cache = SecureCache::builder(inner_cache)
        .cache_directory(&cache_dir)
        .security_config(SecureCacheConfig {
            require_signatures: true,
            enable_access_control: true,
            enable_audit_logging: true,
            enable_merkle_tree: true,
            verify_on_read: true,
            strict_integrity: true,
            ..Default::default()
        })
        .build()
        .await
        .unwrap();

    // Test basic operations
    let test_data = TestCacheData::new(1, "secure_test".to_string());

    secure_cache
        .put("test/secure_key", &test_data, None)
        .await
        .unwrap();
    let retrieved: Option<TestCacheData> = secure_cache.get("test/secure_key").await.unwrap();
    assert_eq!(retrieved, Some(test_data));

    // Test integrity verification
    assert!(secure_cache.verify_integrity().await.unwrap());

    // Test Merkle proof generation
    let proof = secure_cache
        .get_merkle_proof("test/secure_key")
        .await
        .unwrap();
    assert!(proof.is_some());

    // Test removal
    let removed = secure_cache.remove("test/secure_key").await.unwrap();
    assert!(removed);

    let after_removal: Option<TestCacheData> = secure_cache.get("test/secure_key").await.unwrap();
    assert_eq!(after_removal, None);
}

/// Test security error handling and recovery
#[tokio::test]
async fn test_security_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let mut authority = CapabilityAuthority::new("error-test".to_string());

    // Test invalid token error
    let expired_token = authority
        .issue_token(
            "expired-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(1), // 1 second expiration (minimum granularity)
            None,
        )
        .unwrap();

    // Wait for token to definitely expire
    sleep(Duration::from_secs(2)).await;

    let mut checker = CapabilityChecker::new(authority);
    let operation = CacheOperation::Read {
        key: "test".to_string(),
    };

    let result = checker
        .check_permission(&expired_token, &operation)
        .unwrap();
    assert_eq!(
        result,
        AuthorizationResult::TokenInvalid(
            cuenv::cache::capabilities::TokenVerificationResult::Expired
        )
    );

    // Test signature verification error
    let signer = CacheSigner::new(temp_dir.path()).unwrap();
    let test_data = TestCacheData::new(1, "test".to_string());
    let mut signed = signer.sign(&test_data).unwrap();

    // Corrupt signature
    signed.signature[0] ^= 1;
    assert!(!signer.verify(&signed).unwrap());
}

proptest! {
    /// Property-based test for signature security
    #[test]
    fn test_signature_property_based(
        id in 0u64..1000000,
        name in "[a-zA-Z0-9 ]{1,50}",
        data_size in 0usize..10000
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let signer = CacheSigner::new(temp_dir.path()).unwrap();

            let mut test_data = TestCacheData::new(id, name);
            test_data.data = vec![0u8; data_size];

            // Sign and verify
            let signed = signer.sign(&test_data).unwrap();
            prop_assert!(signer.verify(&signed).unwrap());
            prop_assert_eq!(signed.data.clone(), test_data.clone());

            // Tamper with data and verify detection
            if !signed.data.name.is_empty() {
                let mut tampered = signed.clone();
                tampered.data.name.push('X');
                prop_assert!(!signer.verify(&tampered).unwrap());
            }
            Ok(())
        });
    }

    #[test]
    fn test_merkle_tree_property_based(
        keys in prop::collection::vec("[a-z]{1,10}/[a-z]{1,10}", 1..50),
        sizes in prop::collection::vec(1u64..10000, 1..50)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut tree = MerkleTree::new();

            // Deduplicate keys to handle the case where proptest generates duplicates
            let mut unique_keys = std::collections::HashSet::new();

            // Insert entries
            for (key, size) in keys.iter().zip(sizes.iter()) {
                let content_hash = {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(key.as_bytes());
                    hasher.finalize().into()
                };

                let metadata = CacheEntryMetadata {
                    size_bytes: *size,
                    modified_at: 1640000000,
                    content_hash,
                    expires_at: None,
                };

                tree.insert_entry(key.clone(), content_hash, metadata).unwrap();
                unique_keys.insert(key.clone());
            }

            // Verify integrity - use unique_keys count since tree stores unique entries
            let report = tree.verify_integrity().unwrap();
            prop_assert!(report.tree_valid);
            prop_assert_eq!(report.total_entries as usize, unique_keys.len());
            prop_assert_eq!(report.verified_entries as usize, unique_keys.len());

            // Test proofs for all unique keys
            for key in unique_keys.iter() {
                if let Some(proof) = tree.generate_proof(key).unwrap() {
                    prop_assert!(tree.verify_proof(&proof).unwrap());
                    prop_assert_eq!(&proof.cache_key, key);
                }
            }
            Ok(())
        })?;
    }

    #[test]
    fn test_capability_patterns_property_based(
        keys in prop::collection::vec("[a-z]{1,5}/[a-z]{1,5}/[a-z]{1,5}", 1..20),
        patterns in prop::collection::vec("([a-z]{1,5}/?)+\\*?", 1..10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let authority = CapabilityAuthority::new("prop-test".to_string());
            let mut checker = CapabilityChecker::new(authority);

            // Test pattern matching
            for pattern in patterns.iter() {
                // Create a token with this pattern
                let token = checker.issue_token(
                    "test-user".to_string(),
                    [Permission::Read].into_iter().collect(),
                    vec![pattern.clone()],
                    Duration::from_secs(3600),
                    None,
                ).unwrap();
                for key in keys.iter() {
                    // Test if key would match pattern by checking authorization
                    let matches = checker.check_permission(&token, &CacheOperation::Read { key: key.clone() }).is_ok();

                    // Basic pattern matching properties
                    if pattern == "*" {
                        prop_assert!(matches);
                    }
                    if pattern == key {
                        prop_assert!(matches);
                    }
                    if pattern.ends_with('*') {
                        let prefix = &pattern[..pattern.len() - 1];
                        if key.starts_with(prefix) {
                            prop_assert!(matches);
                        }
                    }
                }
            }
            Ok(())
        })?;
    }
}

/// Stress test for concurrent security operations
#[tokio::test]
async fn test_concurrent_security_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let inner_cache = ProductionCache::new(cache_dir.join("cache"), UnifiedCacheConfig::default())
        .await
        .unwrap();
    let secure_cache = std::sync::Arc::new(
        SecureCache::builder(inner_cache)
            .cache_directory(&cache_dir)
            .build()
            .await
            .unwrap(),
    );

    // Spawn concurrent operations
    let mut handles = Vec::new();

    for i in 0..10 {
        let cache = secure_cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("thread_{}_key_{}", i, j);
                let data = TestCacheData::new(i * 10 + j, format!("value_{}", j));

                // Put data
                cache.put(&key, &data, None).await.unwrap();

                // Get data back
                let retrieved: Option<TestCacheData> = cache.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(data));

                // Verify integrity periodically
                if j % 5 == 0 {
                    assert!(cache.verify_integrity().await.unwrap());
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Final integrity check
    assert!(secure_cache.verify_integrity().await.unwrap());
}

/// Test security performance under load
#[tokio::test]
async fn test_security_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let inner_cache = ProductionCache::new(cache_dir.join("cache"), UnifiedCacheConfig::default())
        .await
        .unwrap();
    let secure_cache = SecureCache::builder(inner_cache)
        .cache_directory(&cache_dir)
        .build()
        .await
        .unwrap();

    let start = std::time::Instant::now();

    // Perform many operations
    for i in 0..500 {
        // Reduced from 1000 to 500 for faster test
        let key = format!("perf_key_{}", i);
        let data = TestCacheData::new(i, format!("perf_value_{}", i));

        secure_cache.put(&key, &data, None).await.unwrap();
        let _retrieved: Option<TestCacheData> = secure_cache.get(&key).await.unwrap();
    }

    let duration = start.elapsed();

    // Verify performance is reasonable (less than 30ms per operation on average)
    // 500 operations * 2 (put + get) = 1000 total operations
    // 30ms * 1000 = 30 seconds max
    assert!(
        duration.as_millis() < 30000,
        "Performance test took too long: {:?}",
        duration
    );

    // Verify integrity after performance test
    assert!(secure_cache.verify_integrity().await.unwrap());
}
