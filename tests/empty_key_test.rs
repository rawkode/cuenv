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
        cache.put("valid_key", &vec![4, 5, 6], None),
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
