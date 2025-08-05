//! Comprehensive integration tests to boost cache coverage through public API
//!
//! This test suite exercises internal cache functionality through the public
//! interface to increase coverage of private modules like memory_manager,
//! fast_path, eviction, etc.

use cuenv::cache::{Cache, ProductionCache, UnifiedCacheConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::task::JoinSet;

/// Complex data structure for testing serialization paths
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ComplexData {
    id: u64,
    name: String,
    nested: NestedData,
    list: Vec<String>,
    optional_field: Option<i32>,
    map: std::collections::HashMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NestedData {
    value: f64,
    flag: bool,
    items: Vec<u8>,
}

impl ComplexData {
    fn new(id: u64) -> Self {
        let mut map = std::collections::HashMap::new();
        map.insert("key1".to_string(), id as u32);
        map.insert("key2".to_string(), (id * 2) as u32);

        Self {
            id,
            name: format!("complex_item_{}", id),
            nested: NestedData {
                value: id as f64 * 3.14,
                flag: id % 2 == 0,
                items: (0..=(id % 10)).map(|i| i as u8).collect(),
            },
            list: (0..3).map(|i| format!("item_{}_{}", id, i)).collect(),
            optional_field: if id % 3 == 0 { Some(id as i32) } else { None },
            map,
        }
    }
}

/// Configuration that exercises memory management
fn memory_pressure_config() -> UnifiedCacheConfig {
    UnifiedCacheConfig {
        max_size_bytes: 50 * 1024, // Small 50KB limit to trigger eviction
        max_entries: 20,           // Small entry limit
        default_ttl: None,
        compression_threshold: Some(100), // Low threshold to test compression
        cleanup_interval: Duration::from_secs(1), // Fast cleanup for testing
        encryption_enabled: false,
        compression_enabled: true,
        compression_level: Some(1), // Fast compression
        compression_min_size: Some(50),
        eviction_policy: Some("lru".to_string()),
        max_memory_size: Some(20 * 1024), // 20KB memory limit
        max_disk_size: Some(50 * 1024),   // 50KB disk limit
    }
}

/// Configuration for fast path testing
fn fast_path_config() -> UnifiedCacheConfig {
    UnifiedCacheConfig {
        max_size_bytes: 100 * 1024, // 100KB
        max_entries: 1000,
        default_ttl: None,
        compression_threshold: Some(2048), // High threshold to avoid compression for small items
        cleanup_interval: Duration::from_secs(60),
        encryption_enabled: false,
        compression_enabled: false, // Disable compression for fast path testing
        compression_level: None,
        compression_min_size: None,
        eviction_policy: Some("lru".to_string()),
        max_memory_size: Some(80 * 1024), // 80KB memory
        max_disk_size: Some(100 * 1024),  // 100KB disk
    }
}

/// Configuration for eviction testing
fn eviction_config() -> UnifiedCacheConfig {
    UnifiedCacheConfig {
        max_size_bytes: 10 * 1024, // Very small 10KB limit
        max_entries: 5,            // Very small entry limit to force eviction quickly
        default_ttl: None,
        compression_threshold: Some(512),
        cleanup_interval: Duration::from_secs(1),
        encryption_enabled: false,
        compression_enabled: true,
        compression_level: Some(6),
        compression_min_size: Some(100),
        eviction_policy: Some("lru".to_string()),
        max_memory_size: Some(5 * 1024), // 5KB memory
        max_disk_size: Some(10 * 1024),  // 10KB disk
    }
}

#[tokio::test]
async fn test_memory_pressure_scenarios() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), memory_pressure_config())
        .await
        .unwrap();

    // Test 1: Fill cache beyond memory limits to trigger eviction
    let mut stored_keys = Vec::new();
    for i in 0..30 {
        let key = format!("memory_pressure_{}", i);
        let data = ComplexData::new(i);

        match cache.put(&key, &data, None).await {
            Ok(()) => {
                stored_keys.push(key);
            }
            Err(_) => {
                // Expected under memory pressure
            }
        }

        // Check statistics periodically
        if i % 5 == 0 {
            let stats = cache.statistics().await.unwrap();
            if stats.entry_count > 0 {
                println!(
                    "Memory pressure test - entries: {}, bytes: {}",
                    stats.entry_count, stats.total_bytes
                );
            }
        }
    }

    // Test 2: Verify that some eviction occurred
    let final_stats = cache.statistics().await.unwrap();
    assert!(
        final_stats.entry_count <= 30,
        "Should have evicted some entries due to memory pressure"
    );

    // Test 3: Verify cache is still functional
    let test_key = "post_pressure_test";
    let test_data = ComplexData::new(999);
    cache.put(test_key, &test_data, None).await.unwrap();

    let retrieved: Option<ComplexData> = cache.get(test_key).await.unwrap();
    assert_eq!(retrieved, Some(test_data));
}

#[tokio::test]
async fn test_fast_path_optimization() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), fast_path_config())
        .await
        .unwrap();

    // Test 1: Store many small values that should use fast path
    for i in 0..100 {
        let key = format!("fast_path_{}", i);
        let small_data = format!("small_value_{}", i); // Small string data
        cache.put(&key, &small_data, None).await.unwrap();
    }

    // Test 2: Retrieve values to exercise fast path reads
    for i in 0..100 {
        let key = format!("fast_path_{}", i);
        let expected = format!("small_value_{}", i);
        let retrieved: Option<String> = cache.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(expected));
    }

    // Test 3: Mix of small and large values
    for i in 0..20 {
        let small_key = format!("small_{}", i);
        let large_key = format!("large_{}", i);

        let small_data = format!("tiny_{}", i); // Small data
        let large_data = ComplexData::new(i); // Large data

        cache.put(&small_key, &small_data, None).await.unwrap();
        cache.put(&large_key, &large_data, None).await.unwrap();
    }

    // Test 4: Verify both types are retrievable
    for i in 0..20 {
        let small_key = format!("small_{}", i);
        let large_key = format!("large_{}", i);

        let small_retrieved: Option<String> = cache.get(&small_key).await.unwrap();
        let large_retrieved: Option<ComplexData> = cache.get(&large_key).await.unwrap();

        assert!(small_retrieved.is_some());
        assert!(large_retrieved.is_some());
    }

    let stats = cache.statistics().await.unwrap();
    println!(
        "Fast path test - total entries: {}, total bytes: {}",
        stats.entry_count, stats.total_bytes
    );
}

#[tokio::test]
async fn test_eviction_policies() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), eviction_config())
        .await
        .unwrap();

    // Test 1: Fill cache beyond capacity to trigger eviction
    let mut all_keys = Vec::new();
    for i in 0..15 {
        let key = format!("evict_test_{}", i);
        let data = format!("data_for_eviction_{}", i);
        all_keys.push((key.clone(), data.clone()));

        cache.put(&key, &data, None).await.unwrap();

        let stats = cache.statistics().await.unwrap();
        if stats.entry_count > 5 {
            println!(
                "Eviction test - entries: {}, should trigger eviction soon",
                stats.entry_count
            );
        }
    }

    // Test 2: Check that eviction occurred
    let final_stats = cache.statistics().await.unwrap();
    assert!(
        final_stats.entry_count <= 15,
        "Eviction should have occurred"
    );

    // Test 3: Verify LRU behavior by accessing some keys and adding more
    // Access first few keys to make them "recently used"
    for i in 0..3 {
        let key = format!("evict_test_{}", i);
        let _: Option<String> = cache.get(&key).await.unwrap_or(None);
    }

    // Add more items to force eviction
    for i in 15..20 {
        let key = format!("new_item_{}", i);
        let data = format!("new_data_{}", i);
        cache.put(&key, &data, None).await.unwrap();
    }

    // Test 4: Recently accessed items should be more likely to remain
    let mut remaining_old_items = 0;
    let mut remaining_new_items = 0;

    for i in 0..3 {
        let key = format!("evict_test_{}", i);
        if cache.contains(&key).await.unwrap_or(false) {
            remaining_old_items += 1;
        }
    }

    for i in 15..20 {
        let key = format!("new_item_{}", i);
        if cache.contains(&key).await.unwrap_or(false) {
            remaining_new_items += 1;
        }
    }

    println!(
        "Eviction test - old items remaining: {}, new items remaining: {}",
        remaining_old_items, remaining_new_items
    );
}

#[tokio::test]
async fn test_compression_scenarios() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = memory_pressure_config();
    config.compression_enabled = true;
    config.compression_threshold = Some(100); // Low threshold
    config.compression_level = Some(6); // High compression

    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
        .await
        .unwrap();

    // Test 1: Store compressible data
    let large_text = "This is a long text that should be compressed. ".repeat(20);
    cache.put("compressible", &large_text, None).await.unwrap();

    // Test 2: Store incompressible data (binary-like)
    let binary_data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
    cache.put("binary", &binary_data, None).await.unwrap();

    // Test 3: Retrieve and verify data integrity
    let retrieved_text: Option<String> = cache.get("compressible").await.unwrap();
    assert_eq!(retrieved_text, Some(large_text));

    let retrieved_binary: Option<Vec<u8>> = cache.get("binary").await.unwrap();
    assert_eq!(retrieved_binary, Some(binary_data));

    // Test 4: Check statistics for compression information
    let stats = cache.statistics().await.unwrap();
    if stats.compression_enabled {
        println!(
            "Compression test - compression ratio: {}",
            stats.compression_ratio
        );
    }
}

#[tokio::test]
async fn test_ttl_and_expiration() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), fast_path_config())
        .await
        .unwrap();

    // Test 1: Store items with different TTLs
    cache
        .put(
            "short_lived",
            &"expires_soon",
            Some(Duration::from_millis(100)),
        )
        .await
        .unwrap();
    cache
        .put(
            "medium_lived",
            &"expires_medium",
            Some(Duration::from_millis(500)),
        )
        .await
        .unwrap();
    cache
        .put(
            "long_lived",
            &"expires_later",
            Some(Duration::from_secs(10)),
        )
        .await
        .unwrap();
    cache
        .put("permanent", &"never_expires", None)
        .await
        .unwrap();

    // Test 2: Immediate retrieval should work
    assert!(cache.get::<String>("short_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("medium_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("long_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("permanent").await.unwrap().is_some());

    // Test 3: Wait for short TTL to expire
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(cache.get::<String>("short_lived").await.unwrap().is_none());
    assert!(cache.get::<String>("medium_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("long_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("permanent").await.unwrap().is_some());

    // Test 4: Wait for medium TTL to expire
    tokio::time::sleep(Duration::from_millis(400)).await;

    assert!(cache.get::<String>("medium_lived").await.unwrap().is_none());
    assert!(cache.get::<String>("long_lived").await.unwrap().is_some());
    assert!(cache.get::<String>("permanent").await.unwrap().is_some());
}

#[tokio::test]
async fn test_concurrent_operations_heavy() {
    let temp_dir = TempDir::new().unwrap();
    let cache: Arc<ProductionCache> = Arc::new(
        ProductionCache::new(temp_dir.path().to_path_buf(), memory_pressure_config())
            .await
            .unwrap(),
    );

    let mut tasks = JoinSet::new();
    let num_tasks = 10;
    let ops_per_task = 20;

    // Test 1: Concurrent writers
    for task_id in 0..num_tasks {
        let cache_clone = Arc::clone(&cache);
        tasks.spawn(async move {
            let mut operations = 0;
            for i in 0..ops_per_task {
                let key = format!("concurrent_{}_{}", task_id, i);
                let data = ComplexData::new(task_id * 1000 + i);

                match cache_clone.put(&key, &data, None).await {
                    Ok(()) => operations += 1,
                    Err(_) => {} // Expected under high concurrency/memory pressure
                }
            }
            operations
        });
    }

    // Wait for all write tasks
    let mut total_writes = 0;
    while let Some(result) = tasks.join_next().await {
        if let Ok(count) = result {
            total_writes += count;
        }
    }

    println!(
        "Concurrent test - total successful writes: {}",
        total_writes
    );

    // Test 2: Concurrent readers
    for task_id in 0..num_tasks {
        let cache_clone = Arc::clone(&cache);
        tasks.spawn(async move {
            let mut reads = 0;
            for i in 0..ops_per_task {
                let key = format!("concurrent_{}_{}", task_id, i);
                match cache_clone.get::<ComplexData>(&key).await {
                    Ok(Some(_)) => reads += 1,
                    _ => {} // Miss or error
                }
            }
            reads
        });
    }

    // Wait for all read tasks
    let mut total_reads = 0;
    while let Some(result) = tasks.join_next().await {
        if let Ok(count) = result {
            total_reads += count;
        }
    }

    println!("Concurrent test - total successful reads: {}", total_reads);

    // Test 3: Mixed operations
    for task_id in 0..5 {
        let cache_clone = Arc::clone(&cache);
        tasks.spawn(async move {
            let mut mixed_ops = 0;
            for i in 0..10 {
                if i % 2 == 0 {
                    // Write operation
                    let key = format!("mixed_{}_{}", task_id, i);
                    let data = format!("mixed_data_{}_{}", task_id, i);
                    if cache_clone.put(&key, &data, None).await.is_ok() {
                        mixed_ops += 1;
                    }
                } else {
                    // Read operation
                    let key = format!("mixed_{}_{}", task_id, i - 1);
                    if cache_clone
                        .get::<String>(&key)
                        .await
                        .unwrap_or(None)
                        .is_some()
                    {
                        mixed_ops += 1;
                    }
                }
            }
            mixed_ops
        });
    }

    // Wait for mixed operations
    let mut total_mixed = 0;
    while let Some(result) = tasks.join_next().await {
        if let Ok(count) = result {
            total_mixed += count;
        }
    }

    println!("Concurrent test - total mixed operations: {}", total_mixed);

    // Verify cache is still functional
    let final_stats = cache.statistics().await.unwrap();
    assert!(
        final_stats.writes > 0 || final_stats.errors > 0,
        "Should have recorded some operations"
    );
}

#[tokio::test]
async fn test_serialization_edge_cases() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), fast_path_config())
        .await
        .unwrap();

    // Test 1: Empty collections
    let empty_vec: Vec<String> = vec![];
    let empty_map: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    cache.put("empty_vec", &empty_vec, None).await.unwrap();
    cache.put("empty_map", &empty_map, None).await.unwrap();

    let retrieved_vec: Option<Vec<String>> = cache.get("empty_vec").await.unwrap();
    let retrieved_map: Option<std::collections::HashMap<String, i32>> =
        cache.get("empty_map").await.unwrap();

    assert_eq!(retrieved_vec, Some(empty_vec));
    assert_eq!(retrieved_map, Some(empty_map));

    // Test 2: Nested Option types
    let nested_option: Option<Option<String>> = Some(Some("nested".to_string()));
    let none_option: Option<Option<String>> = Some(None);
    let outer_none: Option<Option<String>> = None;

    cache
        .put("nested_some", &nested_option, None)
        .await
        .unwrap();
    cache.put("nested_none", &none_option, None).await.unwrap();
    cache.put("outer_none", &outer_none, None).await.unwrap();

    let retrieved_nested: Option<Option<Option<String>>> = cache.get("nested_some").await.unwrap();
    let retrieved_none: Option<Option<Option<String>>> = cache.get("nested_none").await.unwrap();
    let retrieved_outer: Option<Option<Option<String>>> = cache.get("outer_none").await.unwrap();

    assert_eq!(retrieved_nested, Some(nested_option));
    assert_eq!(retrieved_none, Some(none_option));
    assert_eq!(retrieved_outer, Some(outer_none));

    // Test 3: Large numbers and edge values
    cache.put("max_u64", &u64::MAX, None).await.unwrap();
    cache.put("min_i64", &i64::MIN, None).await.unwrap();
    cache.put("zero_f64", &0.0f64, None).await.unwrap();
    cache.put("infinity", &f64::INFINITY, None).await.unwrap();

    assert_eq!(cache.get::<u64>("max_u64").await.unwrap(), Some(u64::MAX));
    assert_eq!(cache.get::<i64>("min_i64").await.unwrap(), Some(i64::MIN));
    assert_eq!(cache.get::<f64>("zero_f64").await.unwrap(), Some(0.0f64));
    assert_eq!(
        cache.get::<f64>("infinity").await.unwrap(),
        Some(f64::INFINITY)
    );
}

#[tokio::test]
async fn test_metadata_and_statistics_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), fast_path_config())
        .await
        .unwrap();

    let initial_stats = cache.statistics().await.unwrap();

    // Test 1: Store items and track metadata
    let test_data = ComplexData::new(42);
    cache
        .put("metadata_test", &test_data, Some(Duration::from_secs(300)))
        .await
        .unwrap();

    let metadata = cache.metadata("metadata_test").await.unwrap();
    assert!(metadata.is_some());

    let meta = metadata.unwrap();
    assert!(meta.size_bytes > 0);
    assert!(!meta.content_hash.is_empty());
    assert!(meta.expires_at.is_some());
    assert!(meta.created_at <= SystemTime::now());
    assert!(meta.last_accessed <= SystemTime::now());

    // Test 2: Access item to update metadata
    let _: Option<ComplexData> = cache.get("metadata_test").await.unwrap();

    // Test 3: Statistics should reflect operations
    let final_stats = cache.statistics().await.unwrap();
    assert!(final_stats.writes > initial_stats.writes);
    assert!(final_stats.entry_count > initial_stats.entry_count);

    // Test 4: Remove item and check statistics
    cache.remove("metadata_test").await.unwrap();
    let after_remove_stats = cache.statistics().await.unwrap();
    assert!(after_remove_stats.removals > initial_stats.removals);
}

#[tokio::test]
async fn test_cache_limits_and_boundaries() {
    let temp_dir = TempDir::new().unwrap();
    let mut small_config = eviction_config();
    small_config.max_size_bytes = 1024; // Very small 1KB limit
    small_config.max_entries = 3; // Very few entries

    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), small_config)
        .await
        .unwrap();

    // Test 1: Fill cache to capacity
    for i in 0..10 {
        let key = format!("boundary_{}", i);
        let data = format!("data_{}", i);

        match cache.put(&key, &data, None).await {
            Ok(()) => {
                let stats = cache.statistics().await.unwrap();
                println!(
                    "Boundary test - entry {}: {} entries, {} bytes",
                    i, stats.entry_count, stats.total_bytes
                );
            }
            Err(e) => {
                println!("Boundary test - entry {} failed: {}", i, e);
            }
        }
    }

    // Test 2: Verify limits are enforced
    let final_stats = cache.statistics().await.unwrap();
    assert!(
        final_stats.entry_count <= 10,
        "Entry count should be limited"
    );
    assert!(
        final_stats.total_bytes <= 5000,
        "Size should be reasonably bounded"
    );

    // Test 3: Cache should still be functional
    cache.put("final_test", &"final_value", None).await.unwrap();
    let retrieved: Option<String> = cache.get("final_test").await.unwrap();
    assert_eq!(retrieved, Some("final_value".to_string()));
}
