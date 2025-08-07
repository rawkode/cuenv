#![allow(unused)]
use cuenv::cache::{Cache, CacheError, ProductionCache, UnifiedCacheConfig};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_empty_key_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .unwrap();

    // Test 1: Empty key should be rejected
    let result =
        tokio::time::timeout(Duration::from_secs(2), cache.put("", &vec![1, 2, 3], None)).await;

    match result {
        Ok(Ok(_)) => panic!("Empty key should not be accepted"),
        Ok(Err(CacheError::InvalidKey { .. })) => {
            // This is expected
        }
        Ok(Err(e)) => panic!("Unexpected error for empty key: {}", e),
        Err(_) => panic!("Put with empty key timed out"),
    }

    // Test 2: Cache should still work after empty key rejection
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        cache.put("valid_key", &vec![4u8, 5, 6], None),
    )
    .await;

    match result {
        Ok(Ok(_)) => {
            // Expected
        }
        Ok(Err(e)) => panic!("Valid key rejected after empty key: {}", e),
        Err(_) => panic!("Put with valid key timed out after empty key attempt"),
    }

    // Test 3: Should be able to retrieve the valid key
    let result =
        tokio::time::timeout(Duration::from_secs(2), cache.get::<Vec<u8>>("valid_key")).await;

    match result {
        Ok(Ok(Some(v))) => {
            println!("DEBUG: Expected [4, 5, 6], got {:?}", v);
            assert_eq!(v, vec![4, 5, 6]);
        }
        Ok(Ok(None)) => panic!("Valid key not found"),
        Ok(Err(e)) => panic!("Get failed: {}", e),
        Err(_) => panic!("Get timed out"),
    }
}

#[tokio::test]
async fn test_basic_vec_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .unwrap();

    // Test basic vec serialization without empty key interference
    let test_vec = vec![4u8, 5, 6];
    cache.put("basic_test", &test_vec, None).await.unwrap();

    let retrieved: Option<Vec<u8>> = cache.get("basic_test").await.unwrap();
    println!("DEBUG basic: Expected {:?}, got {:?}", test_vec, retrieved);
    assert_eq!(retrieved, Some(test_vec));
}

#[tokio::test]
async fn test_error_recovery_vec_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .unwrap();

    // First, try an operation that should fail (but not corrupt state)
    let result = cache.put("", &vec![1, 2, 3], None).await;
    assert!(result.is_err(), "Empty key should fail");
    println!("DEBUG empty key error: {:?}", result);

    // Now try a valid operation - this should work correctly
    let test_vec = vec![4u8, 5, 6];
    println!("DEBUG putting: {:?}", test_vec);
    cache
        .put("valid_key_after_error", &test_vec, None)
        .await
        .unwrap();

    let retrieved: Option<Vec<u8>> = cache.get("valid_key_after_error").await.unwrap();
    println!("DEBUG retrieved: {:?}", retrieved);

    // Verify raw bytes if possible
    if let Some(ref v) = retrieved {
        println!("DEBUG retrieved bytes: {:?}", v.as_slice());
        println!("DEBUG expected bytes: {:?}", test_vec.as_slice());
    }

    assert_eq!(retrieved, Some(test_vec));
}

#[tokio::test]
async fn test_timeout_wrapped_error_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .unwrap();

    // Reproduce the exact sequence from the failing test with timeouts
    let result =
        tokio::time::timeout(Duration::from_secs(2), cache.put("", &vec![1, 2, 3], None)).await;

    match result {
        Ok(Ok(_)) => panic!("Empty key should not be accepted"),
        Ok(Err(_)) => {
            println!("DEBUG: Empty key correctly failed");
        }
        Err(_) => panic!("Put with empty key timed out"),
    }

    // Now try the valid key with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        cache.put("valid_key", &vec![4u8, 5, 6], None),
    )
    .await;

    match result {
        Ok(Ok(_)) => {
            println!("DEBUG: Valid put completed");
        }
        Ok(Err(e)) => panic!("Valid key rejected after empty key: {}", e),
        Err(_) => panic!("Put with valid key timed out after empty key attempt"),
    }

    // Get with timeout
    let result =
        tokio::time::timeout(Duration::from_secs(2), cache.get::<Vec<u8>>("valid_key")).await;

    match result {
        Ok(Ok(Some(v))) => {
            println!("DEBUG timeout wrapped: Expected [4, 5, 6], got {:?}", v);
            assert_eq!(v, vec![4, 5, 6]);
        }
        Ok(Ok(None)) => panic!("Valid key not found"),
        Ok(Err(e)) => panic!("Get failed: {}", e),
        Err(_) => panic!("Get timed out"),
    }
}

#[tokio::test]
async fn test_null_byte_key_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .unwrap();

    // Test: Key with null bytes should be rejected
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        cache.put("key\0with\0nulls", &vec![1, 2, 3], None),
    )
    .await;

    match result {
        Ok(Ok(_)) => panic!("Key with null bytes should not be accepted"),
        Ok(Err(CacheError::InvalidKey { .. })) => {
            // This is expected
        }
        Ok(Err(e)) => panic!("Unexpected error for null byte key: {}", e),
        Err(_) => panic!("Put with null byte key timed out"),
    }

    // Cache should still work after null byte key rejection
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        cache.put("valid_key_2", &vec![7, 8, 9], None),
    )
    .await;

    match result {
        Ok(Ok(_)) => {
            // Expected
        }
        Ok(Err(e)) => panic!("Valid key rejected after null byte key: {}", e),
        Err(_) => panic!("Put with valid key timed out after null byte key attempt"),
    }
}
