use cuenv::cache::{Cache, ProductionCache, UnifiedCacheConfig};
use tempfile::TempDir;

/// Regression test for fast-path correctness issue
///
/// Tests that small values (< 256 bytes) are correctly serialized and deserialized
/// when using the fast-path optimization. This test specifically targets the issue
/// where Vec<u8> values were being incorrectly type-inferred as Vec<i32> during
/// serialization.
#[tokio::test]
async fn test_fast_path_vec_u8_correctness() {
    let temp_dir = TempDir::new().unwrap();

    let config = UnifiedCacheConfig {
        compression_enabled: false, // Ensure we're testing the fast path
        ..Default::default()
    };

    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
        .await
        .unwrap();

    // Test various small Vec<u8> values that should use the fast path
    let test_cases = [
        vec![0u8; 10],                // All zeros
        vec![255u8; 10],              // All max values
        vec![1, 2, 3, 4, 5],          // Sequential values
        vec![128, 64, 32, 16],        // Powers of 2
        b"hello world".to_vec(),      // ASCII text
        vec![0xDE, 0xAD, 0xBE, 0xEF], // Hex values
    ];

    for (i, test_data) in test_cases.iter().enumerate() {
        let key = format!("fast_path_test_{i}");

        // Store the value
        cache.put(&key, test_data, None).await.unwrap();

        // Retrieve and verify
        let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        assert_eq!(
            retrieved.as_ref(),
            Some(test_data),
            "Failed for test case {i}: expected {test_data:?}, got {retrieved:?}"
        );
    }

    // Test edge cases around the fast-path threshold (256 bytes)
    let edge_cases = [
        vec![42u8; 255], // Just under threshold
        vec![42u8; 256], // Exactly at threshold
        vec![42u8; 257], // Just over threshold
    ];

    for (i, test_data) in edge_cases.iter().enumerate() {
        let key = format!("edge_case_test_{i}");

        cache.put(&key, test_data, None).await.unwrap();
        let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        assert_eq!(
            retrieved.as_ref(),
            Some(test_data),
            "Failed for edge case {} (size {})",
            i,
            test_data.len()
        );
    }

    // Test that different types work correctly with fast path
    let string_key = "string_test";
    let string_value = "Small string value".to_string();
    cache.put(string_key, &string_value, None).await.unwrap();
    let retrieved_string: Option<String> = cache.get(string_key).await.unwrap();
    assert_eq!(retrieved_string.as_ref(), Some(&string_value));

    let int_key = "int_test";
    let int_value: Vec<i32> = vec![1, 2, 3, 4, 5];
    cache.put(int_key, &int_value, None).await.unwrap();
    let retrieved_int: Option<Vec<i32>> = cache.get(int_key).await.unwrap();
    assert_eq!(retrieved_int.as_ref(), Some(&int_value));
}

/// Test that fast path handles concurrent access correctly
#[tokio::test]
async fn test_fast_path_concurrent_access() {
    let temp_dir = TempDir::new().unwrap();

    let config = UnifiedCacheConfig {
        compression_enabled: false,
        ..Default::default()
    };

    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
        .await
        .unwrap();

    let num_tasks = 10;
    let iterations_per_task = 100;

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let cache_clone = cache.clone();

        let handle = tokio::spawn(async move {
            for i in 0..iterations_per_task {
                let key = format!("concurrent_{task_id}_{i}");
                let value: Vec<u8> = vec![(task_id * 10 + i % 10) as u8; 50];

                cache_clone.put(&key, &value, None).await.unwrap();
                let retrieved: Option<Vec<u8>> = cache_clone.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
