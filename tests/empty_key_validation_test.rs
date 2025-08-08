//! Focused test for empty key validation
//!
//! This test verifies that empty keys are properly rejected
//! and that the cache remains functional after such rejection.

#[cfg(test)]
mod empty_key_tests {
    use cuenv::cache::{Cache, CacheError, ProductionCache};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_empty_key_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // Test 1: Attempt to put with empty key
        let empty_key = "";
        let value = vec![1, 2, 3];

        let result = cache.put(empty_key, &value, None).await;

        // Assert that empty key is rejected with InvalidKey error
        assert!(
            matches!(result, Err(CacheError::InvalidKey { .. })),
            "Empty key should be rejected with InvalidKey error, got: {result:?}"
        );

        // Test 2: Verify cache remains functional after empty key rejection
        let valid_key = "valid_test_key";
        let valid_value = vec![4, 5, 6];

        // This should succeed
        cache
            .put(valid_key, &valid_value, None)
            .await
            .expect("Cache should accept valid key after rejecting empty key");

        // Test 3: Verify we can retrieve the valid value
        let retrieved: Option<Vec<u8>> = cache
            .get(valid_key)
            .await
            .expect("Cache get should work after empty key rejection");

        assert_eq!(
            retrieved,
            Some(valid_value),
            "Cache should correctly store and retrieve values after empty key rejection"
        );

        // Test 4: Attempt to get with empty key
        let get_result = cache.get::<Vec<u8>>(empty_key).await;

        assert!(
            matches!(get_result, Err(CacheError::InvalidKey { .. })),
            "Empty key should be rejected on get operation too, got: {get_result:?}"
        );

        // Test 5: Verify cache is still functional
        let another_key = "another_valid_key";
        let another_value = vec![7, 8, 9];

        cache
            .put(another_key, &another_value, None)
            .await
            .expect("Cache should still be functional");

        let retrieved2: Option<Vec<u8>> = cache
            .get(another_key)
            .await
            .expect("Cache should still be functional");

        assert_eq!(retrieved2, Some(another_value));

        println!("Empty key validation test passed successfully!");
    }

    #[tokio::test]
    async fn test_empty_key_metadata_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // Test metadata operation with empty key
        let empty_key = "";
        let metadata_result = cache.metadata(empty_key).await;

        assert!(
            matches!(metadata_result, Err(CacheError::InvalidKey { .. })),
            "Empty key should be rejected on metadata operation, got: {metadata_result:?}"
        );

        // Test remove operation with empty key
        let remove_result = cache.remove(empty_key).await;

        assert!(
            matches!(remove_result, Err(CacheError::InvalidKey { .. })),
            "Empty key should be rejected on remove operation, got: {remove_result:?}"
        );
    }
}
