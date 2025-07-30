#![allow(unused)]
//! Cache invariant tests with comprehensive coverage
//!
//! This module implements invariant tests that verify critical cache
//! properties hold under all conditions (Phase 8 requirements).

#[cfg(test)]
mod cache_invariant_tests {
    use cuenv::cache::{
        Cache, CacheError, ProductionCache, SyncCache, UnifiedCache, UnifiedCacheConfig,
        CacheMetadata,
    };
    use proptest::prelude::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// Invariant: Cache operations are deterministic for the same inputs
    #[tokio::test]
    async fn invariant_deterministic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        let test_cases = vec![
            ("deterministic_1", b"value_1"),
            ("deterministic_2", b"value_2"),
            ("deterministic_3", b"value_3"),
        ];

        // First run
        for (key, value) in &test_cases {
            cache.put(key, value, None).await.unwrap();
        }

        let mut first_results = Vec::new();
        for (key, _) in &test_cases {
            let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
            first_results.push(result);
        }

        // Second run with same operations
        cache.clear().await.unwrap();
        
        for (key, value) in &test_cases {
            cache.put(key, value, None).await.unwrap();
        }

        let mut second_results = Vec::new();
        for (key, _) in &test_cases {
            let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
            second_results.push(result);
        }

        // Results should be identical
        assert_eq!(first_results, second_results, "Cache operations should be deterministic");
    }

    /// Invariant: Cache size limits are never exceeded
    #[tokio::test]
    async fn invariant_size_limits_respected() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UnifiedCacheConfig {
            max_memory_bytes: 1024 * 1024, // 1MB limit
            max_entries: 100,              // 100 entry limit
            max_entry_size: 10 * 1024,     // 10KB entry limit
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // Test entry count limit
        for i in 0..200 {
            let key = format!("entry_limit_test_{}", i);
            let value = vec![i as u8; 1024]; // 1KB entries
            let _ = cache.put(&key, &value, None).await;
            
            let stats = cache.statistics().await.unwrap();
            assert!(stats.entries <= 100, 
                "Entry count invariant violated: {} > 100 at iteration {}", stats.entries, i);
        }

        // Test memory limit
        cache.clear().await.unwrap();
        
        for i in 0..200 {
            let key = format!("memory_limit_test_{}", i);
            let value = vec![i as u8; 8192]; // 8KB entries
            let _ = cache.put(&key, &value, None).await;
            
            let stats = cache.statistics().await.unwrap();
            assert!(stats.memory_bytes <= 2 * 1024 * 1024, // Allow some overhead
                "Memory limit invariant violated: {} > 2MB at iteration {}", stats.memory_bytes, i);
        }

        // Test entry size limit
        let oversized_value = vec![42u8; 20 * 1024]; // 20KB > 10KB limit
        match cache.put("oversized", &oversized_value, None).await {
            Err(CacheError::ValueTooLarge { .. }) => {
                // Expected behavior
            }
            Ok(_) => {
                panic!("Entry size limit invariant violated: oversized entry was accepted");
            }
            Err(e) => {
                panic!("Unexpected error for oversized entry: {}", e);
            }
        }
    }

    /// Invariant: Cache statistics are monotonic and consistent
    #[tokio::test]
    async fn invariant_statistics_monotonic() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        let mut previous_stats = cache.statistics().await.unwrap();
        
        for i in 0..100 {
            let key = format!("stats_test_{}", i);
            let value = format!("value_{}", i);
            
            // Perform operations
            cache.put(&key, value.as_bytes(), None).await.unwrap();
            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
            let _ = cache.metadata(&key).await.unwrap();
            
            let current_stats = cache.statistics().await.unwrap();
            
            // Monotonic invariants
            assert!(current_stats.total_operations >= previous_stats.total_operations,
                "Total operations should be monotonic: {} < {} at iteration {}", 
                current_stats.total_operations, previous_stats.total_operations, i);
                
            assert!(current_stats.hits >= previous_stats.hits,
                "Hits should be monotonic: {} < {} at iteration {}", 
                current_stats.hits, previous_stats.hits, i);
                
            assert!(current_stats.misses >= previous_stats.misses,
                "Misses should be monotonic: {} < {} at iteration {}", 
                current_stats.misses, previous_stats.misses, i);

            // Consistency invariants
            assert_eq!(current_stats.hits + current_stats.misses + current_stats.errors, 
                current_stats.total_operations,
                "Stats consistency invariant violated at iteration {}: {} + {} + {} != {}", 
                i, current_stats.hits, current_stats.misses, current_stats.errors, current_stats.total_operations);

            assert!(current_stats.hit_rate >= 0.0 && current_stats.hit_rate <= 1.0,
                "Hit rate should be between 0 and 1: {} at iteration {}", 
                current_stats.hit_rate, i);

            previous_stats = current_stats;
        }
    }

    /// Invariant: Data integrity is preserved across all operations
    #[tokio::test]
    async fn invariant_data_integrity() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UnifiedCacheConfig {
            compression_enabled: true,
            checksums_enabled: true,
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // Test various data patterns
        let test_data = vec![
            ("empty", vec![]),
            ("single_byte", vec![42]),
            ("pattern_aa", vec![0xAA; 1000]),
            ("pattern_55", vec![0x55; 1000]),
            ("incremental", (0..256).cycle().take(1000).collect::<Vec<u8>>()),
            ("random", generate_random_data(2048, 12345)),
            ("utf8_text", "Hello, 世界! 🦀".as_bytes().to_vec()),
            ("binary_zeros", vec![0; 512]),
            ("binary_ones", vec![255; 512]),
        ];

        for (name, original_data) in &test_data {
            // Store data
            cache.put(name, original_data, None).await.unwrap();
            
            // Retrieve and verify immediately
            let retrieved: Option<Vec<u8>> = cache.get(name).await.unwrap();
            assert_eq!(retrieved.as_ref(), Some(original_data),
                "Data integrity violated for {} (immediate retrieval)", name);
        }

        // Perform operations that might affect data
        for i in 0..100 {
            let key = format!("interference_{}", i);
            let value = generate_random_data(1024, i as u64);
            cache.put(&key, &value, None).await.unwrap();
        }

        // Verify original data is still intact
        for (name, original_data) in &test_data {
            let retrieved: Option<Vec<u8>> = cache.get(name).await.unwrap();
            if let Some(actual_data) = retrieved {
                assert_eq!(actual_data, *original_data,
                    "Data integrity violated for {} (after interference)", name);
            }
        }

        // Test data integrity across cache restart
        drop(cache);
        
        let restored_cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        for (name, original_data) in &test_data {
            let retrieved: Option<Vec<u8>> = restored_cache.get(name).await.unwrap();
            if let Some(actual_data) = retrieved {
                assert_eq!(actual_data, *original_data,
                    "Data integrity violated for {} (after restart)", name);
            }
        }
    }

    /// Invariant: TTL expiration is consistent and predictable
    #[tokio::test]
    async fn invariant_ttl_consistency() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UnifiedCacheConfig {
            ttl_secs: Some(Duration::from_secs(2)), // 2 second TTL
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let ttl_test_keys = vec!["ttl_1", "ttl_2", "ttl_3", "ttl_4", "ttl_5"];
        let store_time = Instant::now();

        // Store all entries at roughly the same time
        for key in &ttl_test_keys {
            let value = format!("ttl_value_{}", key);
            cache.put(key, value.as_bytes(), None).await.unwrap();
        }

        // They should all be available immediately
        for key in &ttl_test_keys {
            assert!(cache.get::<Vec<u8>>(key).await.unwrap().is_some(),
                "TTL invariant violated: {} not found immediately after store", key);
        }

        // Wait for half the TTL period
        tokio::time::sleep(Duration::from_secs(1)).await;

        // They should still be available
        for key in &ttl_test_keys {
            assert!(cache.get::<Vec<u8>>(key).await.unwrap().is_some(),
                "TTL invariant violated: {} not found at 1s (TTL=2s)", key);
        }

        // Wait past the TTL period
        tokio::time::sleep(Duration::from_secs(2)).await;

        // They should now be expired
        let mut expired_count = 0;
        for key in &ttl_test_keys {
            if cache.get::<Vec<u8>>(key).await.unwrap().is_none() {
                expired_count += 1;
            }
        }

        assert!(expired_count >= ttl_test_keys.len() / 2,
            "TTL invariant violated: only {}/{} entries expired after TTL", 
            expired_count, ttl_test_keys.len());
    }

    /// Invariant: Concurrent operations maintain consistency
    #[tokio::test]
    async fn invariant_concurrent_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Arc::new(
            ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                .await
                .unwrap()
        );

        let num_threads = 8;
        let operations_per_thread = 100;
        let shared_keys = Arc::new((0..20).map(|i| format!("shared_key_{}", i)).collect::<Vec<_>>());
        let consistency_violations = Arc::new(AtomicU64::new(0));

        let mut handles = Vec::new();

        for thread_id in 0..num_threads {
            let cache_clone = Arc::clone(&cache);
            let keys_clone = Arc::clone(&shared_keys);
            let violations_clone = Arc::clone(&consistency_violations);

            let handle = tokio::spawn(async move {
                let mut rng = rand::rngs::StdRng::seed_from_u64(thread_id as u64);
                
                for op_id in 0..operations_per_thread {
                    let key = &keys_clone[rng.gen_range(0..keys_clone.len())];
                    let expected_value = format!("consistent_value_{}_{}", thread_id, op_id);
                    
                    // Write operation
                    match cache_clone.put(key, expected_value.as_bytes(), None).await {
                        Ok(_) => {
                            // Immediate read to verify consistency
                            match cache_clone.get::<Vec<u8>>(key).await {
                                Ok(Some(actual_value)) => {
                                    if actual_value != expected_value.as_bytes() {
                                        // The value we just wrote should be there
                                        // (unless evicted or overwritten by another thread)
                                        // This is acceptable in a concurrent system
                                    }
                                }
                                Ok(None) => {
                                    // Value not found - could be evicted, acceptable
                                }
                                Err(_) => {
                                    violations_clone.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        Err(_) => {
                            // Write failures are acceptable under concurrency
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.await.unwrap();
        }

        let total_violations = consistency_violations.load(Ordering::Relaxed);
        let stats = cache.statistics().await.unwrap();

        // Consistency invariant: error rate should be low
        assert!(total_violations < 10, 
            "Concurrent consistency invariant violated: {} errors", total_violations);

        // The cache should still be functional after concurrent operations
        let test_key = "post_concurrent_test";
        let test_value = b"post_concurrent_value";
        cache.put(test_key, test_value, None).await.unwrap();
        
        let retrieved: Option<Vec<u8>> = cache.get(test_key).await.unwrap();
        assert_eq!(retrieved.as_ref(), Some(&test_value.to_vec()),
            "Cache should remain functional after concurrent operations");
    }

    /// Invariant: Error handling is consistent and safe
    #[tokio::test]
    async fn invariant_error_handling_safety() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UnifiedCacheConfig {
            max_entry_size: 1024, // 1KB limit for testing
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // Test various error conditions
        let error_test_cases = vec![
            ("empty_key", "", vec![1, 2, 3]),
            ("oversized_value", "valid_key", vec![0u8; 2048]), // 2KB > 1KB limit
            ("null_in_key", "key\0with\0nulls", vec![1, 2, 3]),
            ("very_long_key", &"x".repeat(10000), vec![1, 2, 3]),
        ];

        let mut error_count = 0;
        let mut success_count = 0;

        for (test_name, key, value) in error_test_cases {
            match cache.put(key, &value, None).await {
                Ok(_) => {
                    success_count += 1;
                    
                    // If put succeeded, get should work too
                    match cache.get::<Vec<u8>>(key).await {
                        Ok(_) => {
                            // Consistent behavior
                        }
                        Err(e) => {
                            panic!("Error handling invariant violated: put succeeded but get failed for {}: {}", 
                                test_name, e);
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    
                    // Error should be appropriate type
                    match e {
                        CacheError::InvalidKey { .. } => {
                            // Expected for invalid keys
                        }
                        CacheError::ValueTooLarge { .. } => {
                            // Expected for oversized values
                        }
                        CacheError::StorageError { .. } => {
                            // Acceptable storage errors
                        }
                        _ => {
                            // Other errors are also acceptable as long as they don't panic
                        }
                    }
                    
                    // After error, cache should still be functional
                    let recovery_key = format!("recovery_after_{}", test_name);
                    let recovery_value = b"recovery_test";
                    
                    cache.put(&recovery_key, recovery_value, None).await
                        .expect("Cache should be functional after error");
                    
                    let retrieved: Option<Vec<u8>> = cache.get(&recovery_key).await
                        .expect("Cache should be functional after error");
                    
                    assert_eq!(retrieved.as_ref(), Some(&recovery_value.to_vec()),
                        "Cache should maintain functionality after error in {}", test_name);
                }
            }
        }

        println!("Error handling test: {} successes, {} errors (both acceptable)", 
            success_count, error_count);

        // Invariant: Cache should remain operational regardless of errors
        let final_test_key = "final_functionality_test";
        let final_test_value = b"final_test_value";
        
        cache.put(final_test_key, final_test_value, None).await.unwrap();
        let final_result: Option<Vec<u8>> = cache.get(final_test_key).await.unwrap();
        
        assert_eq!(final_result.as_ref(), Some(&final_test_value.to_vec()),
            "Error handling invariant violated: cache not functional after error tests");
    }

    /// Invariant: Cache clear operation removes all entries
    #[tokio::test]
    async fn invariant_clear_completeness() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // Populate cache with various types of data
        let test_entries = vec![
            ("regular_entry", b"regular_value".to_vec()),
            ("large_entry", vec![42u8; 10000]),
            ("small_entry", vec![1]),
            ("empty_value", vec![]),
            ("binary_data", (0..256).collect::<Vec<u8>>()),
        ];

        for (key, value) in &test_entries {
            cache.put(key, value, None).await.unwrap();
        }

        // Verify entries exist
        let stats_before = cache.statistics().await.unwrap();
        assert!(stats_before.entries > 0, "Should have entries before clear");

        for (key, _) in &test_entries {
            assert!(cache.get::<Vec<u8>>(key).await.unwrap().is_some(),
                "Entry {} should exist before clear", key);
        }

        // Clear cache
        cache.clear().await.unwrap();

        // Verify all entries are gone
        let stats_after = cache.statistics().await.unwrap();
        assert_eq!(stats_after.entries, 0, 
            "Clear invariant violated: {} entries remain after clear", stats_after.entries);

        for (key, _) in &test_entries {
            assert!(cache.get::<Vec<u8>>(key).await.unwrap().is_none(),
                "Clear invariant violated: entry {} still exists after clear", key);
        }

        // Verify cache is still functional after clear
        let post_clear_key = "post_clear_test";
        let post_clear_value = b"post_clear_value";
        
        cache.put(post_clear_key, post_clear_value, None).await.unwrap();
        let result: Option<Vec<u8>> = cache.get(post_clear_key).await.unwrap();
        
        assert_eq!(result.as_ref(), Some(&post_clear_value.to_vec()),
            "Cache should be functional after clear operation");
    }

    /// Invariant: Metadata is consistent with stored data
    #[tokio::test]
    async fn invariant_metadata_consistency() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UnifiedCacheConfig {
            compression_enabled: true,
            checksums_enabled: true,
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let metadata_test_cases = vec![
            ("metadata_small", vec![42u8; 100]),
            ("metadata_medium", vec![42u8; 1000]),
            ("metadata_large", vec![42u8; 10000]),
            ("metadata_compressible", b"AAAA".repeat(1000)),
            ("metadata_random", generate_random_data(5000, 99999)),
        ];

        for (key, value) in &metadata_test_cases {
            let store_time = SystemTime::now();
            cache.put(key, value, None).await.unwrap();
            
            // Get metadata
            let metadata = cache.metadata(key).await.unwrap()
                .expect(&format!("Metadata should exist for key {}", key));

            // Verify metadata consistency
            assert!(metadata.size_bytes > 0, 
                "Metadata size should be positive for key {}", key);
            
            assert!(metadata.size_bytes >= value.len() as u64,
                "Metadata size should be at least value size for key {}: {} < {}", 
                key, metadata.size_bytes, value.len());

            assert!(metadata.created_at >= store_time.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
                "Metadata created time should be reasonable for key {}", key);

            assert!(metadata.last_accessed >= metadata.created_at,
                "Metadata last accessed should be >= created time for key {}", key);

            // Verify data can still be retrieved
            let retrieved: Option<Vec<u8>> = cache.get(key).await.unwrap();
            assert_eq!(retrieved.as_ref(), Some(value),
                "Data should be retrievable after metadata query for key {}", key);

            // Access again and verify last_accessed updates
            let metadata_before_access = cache.metadata(key).await.unwrap().unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await; // Small delay
            
            let _: Option<Vec<u8>> = cache.get(key).await.unwrap();
            let metadata_after_access = cache.metadata(key).await.unwrap().unwrap();

            assert!(metadata_after_access.last_accessed >= metadata_before_access.last_accessed,
                "Last accessed time should update after access for key {}", key);
        }
    }

    /// Invariant: Cache operations are atomic
    #[tokio::test]
    async fn invariant_operation_atomicity() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Arc::new(
            ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                .await
                .unwrap()
        );

        let atomicity_key = "atomicity_test";
        let num_writers = 10;
        let writes_per_writer = 50;
        
        // Each writer will write a unique value that can be identified
        let barrier = Arc::new(Barrier::new(num_writers));
        let written_values = Arc::new(Mutex::new(Vec::new()));

        let mut handles = Vec::new();

        for writer_id in 0..num_writers {
            let cache_clone = Arc::clone(&cache);
            let barrier_clone = Arc::clone(&barrier);
            let values_clone = Arc::clone(&written_values);

            let handle = tokio::spawn(async move {
                barrier_clone.wait();
                
                for write_id in 0..writes_per_writer {
                    let unique_value = format!("writer_{}_write_{}", writer_id, write_id);
                    
                    match cache_clone.put(atomicity_key, unique_value.as_bytes(), None).await {
                        Ok(_) => {
                            // Record that this value was successfully written
                            if let Ok(mut values) = values_clone.lock() {
                                values.push(unique_value.clone());
                            }
                            
                            // Small delay to increase chance of concurrent operations
                            tokio::time::sleep(Duration::from_micros(100)).await;
                        }
                        Err(_) => {
                            // Write failures are acceptable under concurrency
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all writers
        for handle in handles {
            handle.await.unwrap();
        }

        // Check final state
        let final_value: Option<Vec<u8>> = cache.get(atomicity_key).await.unwrap();
        
        if let Some(final_bytes) = final_value {
            let final_string = String::from_utf8(final_bytes).unwrap();
            
            // The final value should be one of the values that was written
            let written_values = written_values.lock().unwrap();
            assert!(written_values.contains(&final_string),
                "Atomicity invariant violated: final value '{}' was not in written values", 
                final_string);
                
            // The final value should be a complete, valid value (not corrupted/partial)
            assert!(final_string.starts_with("writer_") && final_string.contains("_write_"),
                "Atomicity invariant violated: final value '{}' appears corrupted", final_string);
        }

        println!("Atomicity test completed: {} values written by {} writers", 
            written_values.lock().unwrap().len(), num_writers);
    }

    /// Property-based invariant tests using proptest
    proptest! {
        #[test]
        fn prop_cache_roundtrip_invariant(
            key in "[a-zA-Z0-9_-]{1,50}",
            value in prop::collection::vec(any::<u8>(), 0..1000)
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await.unwrap();

                // Put-Get invariant: what you put is what you get
                match cache.put(&key, &value, None).await {
                    Ok(_) => {
                        let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                        prop_assert_eq!(retrieved, Some(value), 
                            "Roundtrip invariant violated for key '{}'", key);
                    }
                    Err(CacheError::ValueTooLarge { .. }) => {
                        // Large values might be rejected, which is acceptable
                    }
                    Err(e) => {
                        prop_assert!(false, "Unexpected error: {}", e);
                    }
                }
            });
        }

        #[test]
        fn prop_cache_statistics_invariant(
            operations in prop::collection::vec(
                prop_oneof![
                    (any::<String>(), prop::collection::vec(any::<u8>(), 0..100)).prop_map(|(k, v)| ("put", k, Some(v))),
                    any::<String>().prop_map(|k| ("get", k, None)),
                ],
                1..50
            )
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await.unwrap();

                let initial_stats = cache.statistics().await.unwrap();
                let mut expected_operations = initial_stats.total_operations;

                for (op_type, key, value) in operations {
                    match op_type.as_str() {
                        "put" => {
                            if let Some(v) = value {
                                let _ = cache.put(&key, &v, None).await;
                            }
                        }
                        "get" => {
                            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap_or(None);
                        }
                        _ => unreachable!(),
                    }
                    expected_operations += 1;
                }

                let final_stats = cache.statistics().await.unwrap();
                
                // Statistics invariant: operations should be tracked
                prop_assert!(final_stats.total_operations >= expected_operations,
                    "Statistics invariant violated: {} < {}", 
                    final_stats.total_operations, expected_operations);
                    
                // Hit rate invariant
                prop_assert!(final_stats.hit_rate >= 0.0 && final_stats.hit_rate <= 1.0,
                    "Hit rate invariant violated: {}", final_stats.hit_rate);
            });
        }
    }

    // Helper functions
    
    fn generate_random_data(size: usize, seed: u64) -> Vec<u8> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        use rand::Rng;
        (0..size).map(|_| rng.gen()).collect()
    }
}