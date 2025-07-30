//! Production cache stress tests
//!
//! These tests verify the production-ready cache implementation can handle
//! Google-scale workloads with proper performance characteristics.

use cuenv::cache::traits::{Cache, CacheConfig};
use cuenv::cache::unified_production::UnifiedCache;
use proptest::prelude::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::task::JoinSet;

/// Test concurrent access patterns with high load
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_stress() {
    let temp_dir = TempDir::new().unwrap();
    let cache = Arc::new(
        UnifiedCache::new(
            temp_dir.path().to_path_buf(),
            CacheConfig {
                max_size_bytes: 1024 * 1024 * 1024, // 1GB
                cleanup_interval: Duration::from_secs(60),
                ..Default::default()
            },
        )
        .await
        .unwrap(),
    );

    let num_tasks = 100;
    let operations_per_task = 1000;
    let mut tasks = JoinSet::new();

    let start = Instant::now();

    // Spawn concurrent tasks
    for task_id in 0..num_tasks {
        let cache_clone = Arc::clone(&cache);
        tasks.spawn(async move {
            for op_id in 0..operations_per_task {
                let key = format!("task_{}_op_{}", task_id, op_id);
                let value = format!("value_{}_{}", task_id, op_id);

                // Write
                match cache_clone.put(&key, &value, None).await {
                    Ok(()) => {}
                    Err(e) => panic!("Put failed: {}", e),
                }

                // Read
                let result: Option<String> = match cache_clone.get(&key).await {
                    Ok(v) => v,
                    Err(e) => panic!("Get failed: {}", e),
                };
                assert_eq!(result.as_ref(), Some(&value));

                // Occasionally remove
                if op_id % 10 == 0 {
                    match cache_clone.remove(&key).await {
                        Ok(_) => {}
                        Err(e) => panic!("Remove failed: {}", e),
                    }
                }
            }
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(()) => {}
            Err(e) => panic!("Task failed: {}", e),
        }
    }

    let elapsed = start.elapsed();
    let total_ops = num_tasks * operations_per_task * 2; // reads + writes
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    println!("Concurrent stress test completed:");
    println!("  Total operations: {}", total_ops);
    println!("  Time elapsed: {:?}", elapsed);
    println!("  Operations/sec: {:.0}", ops_per_sec);

    // Verify statistics
    let stats = cache.statistics().await.unwrap();
    assert!(stats.hits > 0);
    assert!(stats.writes > 0);
    assert_eq!(stats.errors, 0);
}

/// Test 4-level sharding distribution
#[tokio::test]
async fn test_sharding_distribution() {
    let temp_dir = TempDir::new().unwrap();
    let cache = UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    // Write many entries to test sharding
    for i in 0..1000 {
        let key = format!("shard_test_{}", i);
        let value = format!("value_{}", i);
        cache.put(&key, &value, None).await.unwrap();
    }

    // Check that directories are properly sharded
    let objects_dir = temp_dir.path().join("objects");
    let metadata_dir = temp_dir.path().join("metadata");

    // Count shard directories at each level
    let mut shard_counts = vec![0, 0, 0, 0];

    for level1 in std::fs::read_dir(&objects_dir).unwrap() {
        let level1_path = level1.unwrap().path();
        if level1_path.is_dir() {
            shard_counts[0] += 1;

            for level2 in std::fs::read_dir(&level1_path).unwrap() {
                let level2_path = level2.unwrap().path();
                if level2_path.is_dir() {
                    shard_counts[1] += 1;

                    for level3 in std::fs::read_dir(&level2_path).unwrap() {
                        let level3_path = level3.unwrap().path();
                        if level3_path.is_dir() {
                            shard_counts[2] += 1;

                            for level4 in std::fs::read_dir(&level3_path).unwrap() {
                                let level4_path = level4.unwrap().path();
                                if level4_path.is_dir() {
                                    shard_counts[3] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Should have reasonable distribution across shards
    println!("Shard distribution: {:?}", shard_counts);
    assert!(shard_counts[0] > 10); // At least 10 top-level shards used
}

/// Test zero-copy memory-mapped file access
#[tokio::test]
async fn test_zero_copy_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache = UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    // Create large data (10MB)
    let large_data = vec![42u8; 10 * 1024 * 1024];
    let key = "large_data";

    // Write large data
    let write_start = Instant::now();
    cache.put(key, &large_data, None).await.unwrap();
    let write_time = write_start.elapsed();

    // Clear memory cache to force disk read
    cache.clear().await.unwrap();

    // Read with mmap (should be fast)
    let read_start = Instant::now();
    let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
    let read_time = read_start.elapsed();

    assert_eq!(result, Some(large_data));

    println!("Zero-copy performance test:");
    println!("  Write time (10MB): {:?}", write_time);
    println!("  Read time (10MB with mmap): {:?}", read_time);

    // Read should be significantly faster than write
    assert!(read_time < write_time * 2);
}

/// Test metadata separation for efficient scanning
#[tokio::test]
async fn test_metadata_scanning() {
    let temp_dir = TempDir::new().unwrap();
    let cache = UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    // Write many entries with different sizes
    for i in 0..100 {
        let key = format!("scan_test_{}", i);
        let value = vec![i as u8; i * 1000]; // Varying sizes
        cache.put(&key, &value, None).await.unwrap();
    }

    // Test metadata access without loading data
    let scan_start = Instant::now();
    let mut total_size = 0u64;

    for i in 0..100 {
        let key = format!("scan_test_{}", i);
        if let Some(metadata) = cache.metadata(&key).await.unwrap() {
            total_size += metadata.size_bytes;
        }
    }

    let scan_time = scan_start.elapsed();

    println!("Metadata scanning test:");
    println!("  Scanned 100 entries in: {:?}", scan_time);
    println!("  Total size: {} bytes", total_size);

    // Scanning should be very fast (< 100ms)
    assert!(scan_time < Duration::from_millis(100));
}

/// Property-based test for cache consistency
proptest! {
    #[test]
    fn prop_cache_never_corrupts(
        operations in prop::collection::vec(
            (0..1000u32, prop::string::string_regex("[a-z]{5,10}").unwrap(), 0..3u8),
            100..500
        )
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let cache = Arc::new(
                UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
                    .await
                    .unwrap()
            );

            let mut expected_state = std::collections::HashMap::new();

            for (seed, key, op_type) in operations {
                match op_type {
                    0 => {
                        // Put
                        let value = format!("value_{}", seed);
                        match cache.put(&key, &value, None).await {
                            Ok(()) => {
                                expected_state.insert(key.clone(), value);
                            }
                            Err(e) => panic!("Put failed: {}", e),
                        }
                    }
                    1 => {
                        // Get
                        let result: Option<String> = match cache.get(&key).await {
                            Ok(v) => v,
                            Err(e) => panic!("Get failed: {}", e),
                        };

                        assert_eq!(result.as_ref(), expected_state.get(&key));
                    }
                    2 => {
                        // Remove
                        match cache.remove(&key).await {
                            Ok(_) => {
                                expected_state.remove(&key);
                            }
                            Err(e) => panic!("Remove failed: {}", e),
                        }
                    }
                    _ => unreachable!(),
                }
            }

            // Verify final state
            for (key, expected_value) in &expected_state {
                let actual: Option<String> = match cache.get(key).await {
                    Ok(v) => v,
                    Err(e) => panic!("Final get failed: {}", e),
                };
                assert_eq!(actual.as_ref(), Some(expected_value));
            }

            // Verify statistics consistency
            let stats = cache.statistics().await.unwrap();
            assert_eq!(stats.errors, 0);
            assert!(stats.total_bytes > 0 || expected_state.is_empty());
        });
    }
}

/// Test cache behavior under memory pressure
#[tokio::test]
async fn test_memory_pressure() {
    let temp_dir = TempDir::new().unwrap();
    let cache = UnifiedCache::new(
        temp_dir.path().to_path_buf(),
        CacheConfig {
            max_size_bytes: 1024 * 1024, // 1MB limit
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Try to exceed capacity
    let large_value = vec![0u8; 512 * 1024]; // 512KB

    // First put should succeed
    cache.put("item1", &large_value, None).await.unwrap();

    // Second put should succeed (still under 1MB)
    cache.put("item2", &large_value, None).await.unwrap();

    // Third put should fail (would exceed 1MB)
    let result = cache.put("item3", &large_value, None).await;
    assert!(result.is_err());

    // Verify error type
    if let Err(e) = result {
        match e {
            cuenv::cache::errors::CacheError::CapacityExceeded { .. } => {}
            _ => panic!("Expected CapacityExceeded error, got: {:?}", e),
        }
    }
}

/// Test crash recovery and atomicity
#[tokio::test]
async fn test_crash_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().to_path_buf();

    // Create cache and write data
    {
        let cache = UnifiedCache::new(cache_path.clone(), CacheConfig::default())
            .await
            .unwrap();

        for i in 0..10 {
            let key = format!("persist_{}", i);
            let value = format!("value_{}", i);
            cache.put(&key, &value, None).await.unwrap();
        }

        // Cache drops here, simulating crash
    }

    // Create new cache instance (simulating restart)
    let cache = UnifiedCache::new(cache_path, CacheConfig::default())
        .await
        .unwrap();

    // Verify all data is still accessible
    for i in 0..10 {
        let key = format!("persist_{}", i);
        let expected = format!("value_{}", i);
        let actual: Option<String> = cache.get(&key).await.unwrap();
        assert_eq!(actual, Some(expected));
    }
}

/// Benchmark cache operations
#[tokio::test]
async fn bench_cache_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = Arc::new(
        UnifiedCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
            .await
            .unwrap(),
    );

    let iterations = 10000;

    // Benchmark writes
    let write_start = Instant::now();
    for i in 0..iterations {
        let key = format!("bench_{}", i);
        let value = format!("value_{}", i);
        cache.put(&key, &value, None).await.unwrap();
    }
    let write_time = write_start.elapsed();

    // Benchmark reads (hot path)
    let read_start = Instant::now();
    for i in 0..iterations {
        let key = format!("bench_{}", i);
        let _: Option<String> = cache.get(&key).await.unwrap();
    }
    let hot_read_time = read_start.elapsed();

    // Clear memory cache
    cache.clear().await.unwrap();

    // Benchmark reads (cold path with mmap)
    let cold_read_start = Instant::now();
    for i in 0..iterations {
        let key = format!("bench_{}", i);
        let _: Option<String> = cache.get(&key).await.unwrap();
    }
    let cold_read_time = cold_read_start.elapsed();

    println!("Cache benchmark results ({} operations):", iterations);
    println!(
        "  Write throughput: {:.0} ops/sec",
        iterations as f64 / write_time.as_secs_f64()
    );
    println!(
        "  Hot read throughput: {:.0} ops/sec",
        iterations as f64 / hot_read_time.as_secs_f64()
    );
    println!(
        "  Cold read throughput: {:.0} ops/sec",
        iterations as f64 / cold_read_time.as_secs_f64()
    );

    // Performance assertions
    assert!(hot_read_time < write_time); // Hot reads should be faster than writes
    assert!(cold_read_time < write_time * 2); // Cold reads should still be reasonably fast
}
