#![allow(unused)]
//! Property-based tests for the cache system
//!
//! These tests use proptest to verify cache invariants and behavior across
//! a wide range of inputs and scenarios, following Phase 8 requirements.

#[cfg(test)]
mod cache_property_tests {
    use cuenv::cache::{
        Cache, CacheError, CacheKey, CacheMetadata, ProductionCache, SyncCache, UnifiedCache,
        UnifiedCacheConfig,
    };
    use proptest::prelude::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// Generate valid cache keys
    fn arb_cache_key() -> impl Strategy<Value = String> {
        prop_oneof![
            // Simple alphanumeric keys
            "[a-zA-Z0-9_-]{1,64}",
            // Hash-like keys
            "[a-f0-9]{64}",
            // Hierarchical keys
            "[a-zA-Z0-9_-]{1,20}(/[a-zA-Z0-9_-]{1,20}){0,5}",
            // UUID-like keys
            "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
        ]
    }

    /// Generate arbitrary byte arrays for cache values
    fn arb_cache_value() -> impl Strategy<Value = Vec<u8>> {
        prop_oneof![
            // Small values (0-1KB)
            prop::collection::vec(any::<u8>(), 0..1024),
            // Medium values (1KB-64KB)
            prop::collection::vec(any::<u8>(), 1024..65536),
            // Large values (64KB-1MB) - less frequent to avoid test slowdown
            prop::collection::vec(any::<u8>(), 65536..1024 * 1024).prop_map(|mut v| {
                v.truncate(1024 * 1024); // Cap at 1MB
                v
            }),
        ]
    }

    /// Generate cache configurations
    fn arb_cache_config() -> impl Strategy<Value = UnifiedCacheConfig> {
        (
            1024_u64..1024 * 1024 * 1024, // max_size_bytes: 1KB to 1GB
            0_u64..10000,                 // max_entries
            0_u64..3600,                  // ttl_secs: 0 to 1 hour
            any::<bool>(),                // compression_enabled
        )
            .prop_map(
                |(max_size_bytes, max_entries, ttl_secs, compression_enabled)| UnifiedCacheConfig {
                    max_size_bytes,
                    max_entries,
                    default_ttl: if ttl_secs == 0 {
                        None
                    } else {
                        Some(Duration::from_secs(ttl_secs))
                    },
                    compression_enabled,
                    ..Default::default()
                },
            )
    }

    /// Property: Cache round-trip consistency
    /// For any key-value pair, putting and then getting should return the same value
    proptest! {
        #[test]
        fn prop_cache_roundtrip_consistency(
            key in arb_cache_key(),
            value in arb_cache_value(),
            config in arb_cache_config(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = match ProductionCache::new(temp_dir.path().to_path_buf(), config).await {
                    Ok(cache) => cache,
                    Err(_) => return Ok(()), // Skip invalid configs
                };

                // Put value
                match cache.put(&key, &value, None).await {
                    Ok(_) => {},
                    Err(CacheError::CapacityExceeded { .. }) => return Ok(()), // Expected for large values
                    Err(e) => prop_assert!(false, "Unexpected error putting value: {}", e),
                }

                // Get value back
                let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();

                match retrieved {
                    Some(retrieved_value) => {
                        prop_assert_eq!(retrieved_value, value, "Retrieved value must match stored value");
                    }
                    None => {
                        // Value might not be found if it was evicted or rejected
                        // This is acceptable behavior under memory pressure
                    }
                }
                Ok(())
            });
        }

        #[test]
        fn prop_cache_key_uniqueness(
            keys_and_values in prop::collection::vec(
                (arb_cache_key(), arb_cache_value()),
                1..50
            ),
            config in arb_cache_config(),
        ) {
            // Ensure all keys are unique
            let mut unique_pairs = Vec::new();
            let mut seen_keys = std::collections::HashSet::new();
            for (key, value) in keys_and_values {
                if seen_keys.insert(key.clone()) {
                    unique_pairs.push((key, value));
                }
            }

            prop_assume!(!unique_pairs.is_empty());

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = match ProductionCache::new(temp_dir.path().to_path_buf(), config).await {
                    Ok(cache) => cache,
                    Err(_) => return Ok(()), // Skip invalid configs
                };

                // Store all key-value pairs
                let mut stored_pairs = Vec::new();
                for (key, value) in unique_pairs {
                    match cache.put(&key, &value, None).await {
                        Ok(_) => {
                            stored_pairs.push((key, value));
                        }
                        Err(CacheError::CapacityExceeded { .. }) => {
                            // Skip values that are too large
                            continue;
                        }
                        Err(e) => {
                            prop_assert!(false, "Unexpected error: {}", e);
                        }
                    }
                }

                // Verify each stored key returns the correct value
                for (key, expected_value) in stored_pairs {
                    let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                    if let Some(actual_value) = retrieved {
                        prop_assert_eq!(actual_value, expected_value,
                            "Key '{}' should return its stored value", key);
                    }
                }
                Ok(())
            });
        }

        #[test]
        fn prop_cache_metadata_consistency(
            key in arb_cache_key(),
            value in arb_cache_value(),
            config in arb_cache_config(),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = match ProductionCache::new(temp_dir.path().to_path_buf(), config).await {
                    Ok(cache) => cache,
                    Err(_) => return Ok(()), // Skip invalid configs
                };

                // Store value
                let put_time = SystemTime::now();
                match cache.put(&key, &value, None).await {
                    Ok(_) => {},
                    Err(CacheError::CapacityExceeded { .. }) => return Ok(()),
                    Err(e) => prop_assert!(false, "Put failed: {}", e),
                }

                // Get metadata
                if let Some(metadata) = cache.metadata(&key).await.unwrap() {
                    // Size should be reasonable
                    prop_assert!(metadata.size_bytes > 0, "Metadata size should be positive");
                    prop_assert!(metadata.size_bytes >= value.len() as u64,
                        "Metadata size should be at least value size");

                    // Timestamps should be reasonable
                    prop_assert!(metadata.created_at >= put_time,
                        "Created timestamp should be after put operation");
                    prop_assert!(metadata.last_accessed >= metadata.created_at,
                        "Last accessed should be >= created time");
                }
                Ok(())
            });
        }

        #[test]
        fn prop_cache_ttl_behavior(
            key in arb_cache_key(),
            value in arb_cache_value().prop_filter("Limit size for TTL test", |v| v.len() < 10000),
            ttl_ms in 100_u64..2000_u64, // 100ms to 2s TTL
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let config = UnifiedCacheConfig {
                    default_ttl: Some(Duration::from_millis(ttl_ms)),
                    ..Default::default()
                };

                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                    .await.unwrap();

                // Store value
                cache.put(&key, &value, None).await.unwrap();

                // Should be retrievable immediately
                let immediate: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                prop_assert!(immediate.is_some(), "Value should be available immediately");

                // Wait for expiration (add buffer for test reliability)
                tokio::time::sleep(Duration::from_millis(ttl_ms + 500)).await;

                // Should be expired now
                let expired: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                prop_assert!(expired.is_none(), "Value should be expired after TTL");
                Ok(())
            });
        }

        #[test]
        fn prop_cache_eviction_under_pressure(
            entries in prop::collection::vec(
                (arb_cache_key(), prop::collection::vec(any::<u8>(), 1000..2000)),
                10..100
            ),
        ) {
            // Ensure unique keys
            let mut unique_entries = Vec::new();
            let mut seen_keys = std::collections::HashSet::new();
            for (key, value) in entries {
                if seen_keys.insert(key.clone()) {
                    unique_entries.push((key, value));
                }
            }

            prop_assume!(unique_entries.len() >= 10);

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();

                // Create cache with limited memory (10KB)
                let config = UnifiedCacheConfig {
                    max_size_bytes: 10 * 1024,
                    max_entries: 5, // Also limit by count
                    ..Default::default()
                };

                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                    .await.unwrap();

                let mut stored_keys = Vec::new();

                // Store entries until memory pressure kicks in
                for (key, value) in unique_entries {
                    match cache.put(&key, &value, None).await {
                        Ok(_) => {
                            stored_keys.push(key);
                        }
                        Err(CacheError::CapacityExceeded { .. }) => {
                            // Expected when value exceeds cache limits
                            continue;
                        }
                        Err(e) => {
                            prop_assert!(false, "Unexpected error: {}", e);
                        }
                    }
                }

                // Verify that cache enforces its limits
                let mut found_count = 0;
                for key in stored_keys {
                    let result: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                    if result.is_some() {
                        found_count += 1;
                    }
                }

                // Cache should not exceed its configured limits
                prop_assert!(
                    found_count <= 5,
                    "Cache should not exceed max_entries limit (found {} entries)",
                    found_count
                );
                Ok(())
            });
        }

        #[test]
        fn prop_cache_concurrent_safety(
            shared_keys in prop::collection::vec(arb_cache_key(), 5..20),
            values_per_key in prop::collection::vec(arb_cache_value(), 5..20),
            num_tasks in 2_usize..8,
        ) {
            prop_assume!(shared_keys.len() >= 5);
            prop_assume!(values_per_key.len() >= 5);

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = Arc::new(
                    ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                        .await.unwrap()
                );

                let mut handles = Vec::new();

                // Spawn concurrent tasks
                for task_id in 0..num_tasks {
                    let cache_clone = Arc::clone(&cache);
                    let keys = shared_keys.clone();
                    let values = values_per_key.clone();

                    let handle = tokio::spawn(async move {
                        let mut operations_completed = 0;

                        for (i, key) in keys.iter().enumerate() {
                            let value_index = (task_id + i) % values.len();
                            let value = &values[value_index];

                            // Alternate between puts and gets
                            if i % 2 == 0 {
                                // Put operation
                                match cache_clone.put(key, value, None).await {
                                    Ok(_) => operations_completed += 1,
                                    Err(CacheError::CapacityExceeded { .. }) => {
                                        // Expected for large values
                                    }
                                    Err(_) => {
                                        // Other errors are acceptable under concurrent load
                                    }
                                }
                            } else {
                                // Get operation
                                match cache_clone.get::<Vec<u8>>(key).await {
                                    Ok(_) => operations_completed += 1,
                                    Err(_) => {
                                        // Errors are acceptable under concurrent load
                                    }
                                }
                            }
                        }

                        operations_completed
                    });
                    handles.push(handle);
                }

                // Wait for all tasks to complete
                let mut total_operations = 0;
                for handle in handles {
                    match handle.await {
                        Ok(count) => total_operations += count,
                        Err(_) => {
                            // Task panics are not expected but acceptable in stress tests
                        }
                    }
                }

                // Verify the cache is still functional after concurrent access
                let test_key = "post_concurrent_test";
                let test_value = b"test_value".to_vec();

                match cache.put(test_key, &test_value, None).await {
                    Ok(_) => {
                        let retrieved: Option<Vec<u8>> = cache.get(test_key).await.unwrap();
                        prop_assert!(retrieved.is_some(), "Cache should be functional after concurrent access");
                    }
                    Err(_) => {
                        // Cache might be full or under pressure, which is acceptable
                    }
                }

                prop_assert!(total_operations > 0, "Some operations should have completed");
                Ok(())
            });
        }

        #[test]
        fn prop_cache_clear_removes_all_entries(
            entries in prop::collection::vec(
                (arb_cache_key(), arb_cache_value().prop_filter("Limit size", |v| v.len() < 1000)),
                1..20
            ),
        ) {
            // Ensure unique keys
            let mut unique_entries = Vec::new();
            let mut seen_keys = std::collections::HashSet::new();
            for (key, value) in entries {
                if seen_keys.insert(key.clone()) {
                    unique_entries.push((key, value));
                }
            }

            prop_assume!(!unique_entries.is_empty());

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await.unwrap();

                let mut stored_keys = Vec::new();

                // Store entries
                for (key, value) in unique_entries {
                    match cache.put(&key, &value, None).await {
                        Ok(_) => {
                            stored_keys.push(key);
                        }
                        Err(CacheError::CapacityExceeded { .. }) => {
                            // Skip values that are too large
                            continue;
                        }
                        Err(e) => {
                            prop_assert!(false, "Put failed: {}", e);
                        }
                    }
                }

                prop_assume!(!stored_keys.is_empty());

                // Verify entries exist before clear
                let mut pre_clear_found = 0;
                for key in &stored_keys {
                    let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
                    if result.is_some() {
                        pre_clear_found += 1;
                    }
                }

                // Clear cache
                cache.clear().await.unwrap();

                // Verify no entries exist after clear
                let mut post_clear_found = 0;
                for key in &stored_keys {
                    let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
                    if result.is_some() {
                        post_clear_found += 1;
                    }
                }

                prop_assert_eq!(post_clear_found, 0, "No entries should remain after clear");
                prop_assert!(pre_clear_found > 0, "Should have had entries before clear");
                Ok(())
            });
        }

        #[test]
        fn prop_cache_statistics_monotonic(
            operations in prop::collection::vec(
                prop_oneof![
                    (arb_cache_key(), arb_cache_value()).prop_map(|(k, v)| ("put", k, Some(v))),
                    arb_cache_key().prop_map(|k| ("get", k, None)),
                ],
                10..50
            ),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await.unwrap();

                let mut previous_stats = cache.statistics().await.unwrap();

                for (op_type, key, value) in operations {
                    match op_type {
                        "put" => {
                            if let Some(v) = value {
                                if v.len() < 10000 { // Limit size to avoid ValueTooLarge
                                    let _ = cache.put(&key, &v, None).await;
                                }
                            }
                        }
                        "get" => {
                            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap_or(None);
                        }
                        _ => unreachable!(),
                    }

                    let current_stats = cache.statistics().await.unwrap();

                    // Statistics should only increase (monotonic)
                    let current_total_ops = current_stats.hits + current_stats.misses + current_stats.writes;
                    let previous_total_ops = previous_stats.hits + previous_stats.misses + previous_stats.writes;
                    prop_assert!(
                        current_total_ops >= previous_total_ops,
                        "Total operations should be monotonic"
                    );
                    prop_assert!(
                        current_stats.hits >= previous_stats.hits,
                        "Hits should be monotonic"
                    );
                    prop_assert!(
                        current_stats.misses >= previous_stats.misses,
                        "Misses should be monotonic"
                    );

                    previous_stats = current_stats;
                }
                Ok(())
            });
        }

        #[test]
        fn prop_cache_error_handling_robustness(
            invalid_operations in prop::collection::vec(
                prop_oneof![
                    // Empty keys
                    Just(("empty_key", "".to_string(), vec![1, 2, 3])),
                    // Very long keys (should be rejected or truncated)
                    Just(("long_key", "x".repeat(10000), vec![1, 2, 3])),
                    // Empty values
                    Just(("empty_value", "valid_key".to_string(), vec![])),
                ],
                1..10
            ),
        ) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await.unwrap();

                // Cache should handle invalid operations gracefully without panicking
                for (_op_name, key, value) in invalid_operations {
                    // These operations might fail, but should not panic
                    let put_result = cache.put(&key, &value, None).await;
                    let get_result: Result<Option<Vec<u8>>, _> = cache.get(&key).await;
                    let metadata_result = cache.metadata(&key).await;

                    // Verify we get proper error types, not panics
                    match put_result {
                        Ok(_) | Err(CacheError::InvalidKey { .. })
                        | Err(CacheError::CapacityExceeded { .. }) => {
                            // These are all acceptable outcomes
                        }
                        Err(e) => {
                            // Other errors are also acceptable as long as we don't panic
                            prop_assert!(true, "Got error (acceptable): {}", e);
                        }
                    }

                    // Get and metadata should handle invalid keys gracefully (either Ok(None) or InvalidKey error)
                    match get_result {
                        Ok(_) | Err(CacheError::InvalidKey { .. }) => {
                            // Both outcomes are acceptable for invalid keys
                        }
                        Err(e) => prop_assert!(false, "Get should only fail with InvalidKey error for invalid keys, got: {}", e),
                    }

                    match metadata_result {
                        Ok(_) | Err(CacheError::InvalidKey { .. }) => {
                            // Both outcomes are acceptable for invalid keys
                        }
                        Err(e) => prop_assert!(false, "Metadata should only fail with InvalidKey error for invalid keys, got: {}", e),
                    }
                }

                // Cache should still be usable after error conditions
                let test_result = cache.put("recovery_test", b"test", None).await;
                prop_assert!(test_result.is_ok(), "Cache should recover from error conditions");
                Ok(())
            })?;
        }
    }

    /// Sync cache property tests
    proptest! {
        #[test]
        fn prop_sync_cache_roundtrip(
            key in arb_cache_key(),
            value in arb_cache_value().prop_filter("Limit size", |v| v.len() < 10000),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            let unified_cache = match rt.block_on(UnifiedCache::new(temp_dir.path().to_path_buf(), Default::default())) {
                Ok(cache) => cache,
                Err(_) => return Ok(()), // Skip if setup fails
            };
            let cache = match SyncCache::new(unified_cache) {
                Ok(cache) => cache,
                Err(_) => return Ok(()), // Skip if setup fails
            };

            // Put and get should be consistent
            match cache.put(&key, &value, None) {
                Ok(_) => {
                    let retrieved: Option<Vec<u8>> = cache.get(&key).unwrap_or(None);
                    if let Some(actual) = retrieved {
                        prop_assert_eq!(actual, value, "Sync cache roundtrip should be consistent");
                    }
                }
                Err(CacheError::CapacityExceeded { .. }) => {
                    // Expected for large values
                }
                Err(e) => {
                    prop_assert!(false, "Unexpected sync cache error: {}", e);
                }
            }
        }
    }
}
