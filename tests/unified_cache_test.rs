//! Comprehensive tests for the unified cache implementation

use cuenv::cache::{Cache, CacheBuilder, CacheError, CacheKey, RecoveryHint, UnifiedCacheConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::sync::Barrier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u64,
    name: String,
    data: Vec<u8>,
    timestamp: SystemTime,
}

impl TestData {
    fn new(id: u64, size: usize) -> Self {
        Self {
            id,
            name: format!("test-{}", id),
            data: vec![id as u8; size],
            timestamp: SystemTime::now(),
        }
    }
}

#[tokio::test]
async fn test_basic_cache_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Test put and get
    let key = "test-key";
    let value = TestData::new(1, 100);

    cache.put(key, &value, None).await.unwrap();
    let retrieved: Option<TestData> = cache.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.clone()));

    // Test contains
    assert!(cache.contains(key).await.unwrap());
    assert!(!cache.contains("non-existent").await.unwrap());

    // Test remove
    assert!(cache.remove(key).await.unwrap());
    assert!(!cache.contains(key).await.unwrap());
    assert!(!cache.remove(key).await.unwrap()); // Already removed

    // Test metadata
    cache.put(key, &value, None).await.unwrap();
    let metadata = cache.metadata(key).await.unwrap().unwrap();
    assert!(metadata.size_bytes > 100); // At least 100 bytes
    assert!(metadata.content_hash.len() > 0);
}

#[tokio::test]
async fn test_cache_expiration() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    let key = "expiring-key";
    let value = TestData::new(1, 50);

    // Put with short TTL
    cache
        .put(key, &value, Some(Duration::from_millis(100)))
        .await
        .unwrap();

    // Should exist immediately
    assert!(cache.contains(key).await.unwrap());

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Should be expired
    let retrieved: Option<TestData> = cache.get(key).await.unwrap();
    assert_eq!(retrieved, None);

    // Also check contains returns false for expired entries
    assert!(!cache.contains(key).await.unwrap());
}

#[tokio::test]
async fn test_cache_capacity_limits() {
    let temp_dir = TempDir::new().unwrap();
    let config = UnifiedCacheConfig {
        max_size_bytes: 1024, // 1KB limit
        ..Default::default()
    };

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(config)
        .build_async()
        .await
        .unwrap();

    // Store values that fit
    for i in 0..5 {
        let key = format!("small-{}", i);
        let value = TestData::new(i, 50); // Small values
        cache.put(&key, &value, None).await.unwrap();
    }

    // Try to store a value that exceeds remaining capacity
    let large_value = TestData::new(999, 2000);
    let result = cache.put("large", &large_value, None).await;

    match result {
        Err(CacheError::CapacityExceeded { .. }) => {
            // Expected
        }
        _ => panic!("Expected CapacityExceeded error"),
    }
}

#[tokio::test]
async fn test_concurrent_access() {
    let temp_dir = TempDir::new().unwrap();
    let cache = Arc::new(
        CacheBuilder::new(temp_dir.path())
            .build_async()
            .await
            .unwrap(),
    );

    let num_tasks = 100;
    let barrier = Arc::new(Barrier::new(num_tasks));

    let handles: Vec<_> = (0..num_tasks)
        .map(|i| {
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);

            tokio::spawn(async move {
                // Synchronize all tasks to start together
                barrier.wait().await;

                let key = format!("concurrent-{}", i % 10); // Reuse some keys
                let value = TestData::new(i as u64, 100);

                // Perform multiple operations
                for _ in 0..10 {
                    cache.put(&key, &value, None).await.unwrap();
                    let _: Option<TestData> = cache.get(&key).await.unwrap();
                    cache.contains(&key).await.unwrap();
                }
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify cache is in consistent state
    let stats = cache.statistics().await.unwrap();
    assert!(stats.writes > 0);
    assert!(stats.hits > 0);
    assert!(stats.errors == 0);
}

#[tokio::test]
async fn test_batch_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Prepare batch data
    let entries: Vec<_> = (0..10)
        .map(|i| {
            let key = format!("batch-{}", i);
            let value = TestData::new(i, 50);
            let ttl = if i % 2 == 0 {
                Some(Duration::from_secs(60))
            } else {
                None
            };
            (key, value, ttl)
        })
        .collect();

    // Batch put
    cache.put_many(&entries).await.unwrap();

    // Batch get
    let keys: Vec<_> = entries.iter().map(|(k, _, _)| k.clone()).collect();
    let results = cache.get_many::<TestData>(&keys).await.unwrap();

    assert_eq!(results.len(), keys.len());
    for (i, (key, value)) in results.iter().enumerate() {
        assert_eq!(key, &format!("batch-{}", i));
        assert!(value.is_some());
        assert_eq!(value.as_ref().unwrap().id, i as u64);
    }
}

#[tokio::test]
async fn test_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Test invalid key
    let result = cache.get::<TestData>("").await;
    match result {
        Err(CacheError::InvalidKey { .. }) => {
            // Expected
        }
        _ => panic!("Expected InvalidKey error"),
    }

    // Test key with null bytes
    let result = cache.get::<TestData>("key\0with\0nulls").await;
    match result {
        Err(CacheError::InvalidKey { reason, .. }) => {
            assert!(reason.contains("null"));
        }
        _ => panic!("Expected InvalidKey error"),
    }

    // Test very long key
    let long_key = "x".repeat(2000);
    let result = cache.get::<TestData>(&long_key).await;
    match result {
        Err(CacheError::InvalidKey { reason, .. }) => {
            assert!(reason.contains("length"));
        }
        _ => panic!("Expected InvalidKey error"),
    }
}

#[tokio::test]
async fn test_cache_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().to_path_buf();

    // Create cache and store data
    {
        let cache = CacheBuilder::new(&cache_path).build_async().await.unwrap();

        for i in 0..10 {
            let key = format!("persist-{}", i);
            let value = TestData::new(i, 100);
            cache.put(&key, &value, None).await.unwrap();
        }
    }

    // Create new cache instance with same path
    {
        let cache = CacheBuilder::new(&cache_path).build_async().await.unwrap();

        // Data should be persisted
        for i in 0..10 {
            let key = format!("persist-{}", i);
            let value: Option<TestData> = cache.get(&key).await.unwrap();
            assert!(value.is_some());
            assert_eq!(value.unwrap().id, i);
        }
    }
}

#[test]
fn test_sync_cache_wrapper() {
    let temp_dir = TempDir::new().unwrap();
    let sync_cache = CacheBuilder::new(temp_dir.path()).build_sync().unwrap();

    // Test basic operations synchronously
    let key = "sync-key";
    let value = TestData::new(1, 50);

    sync_cache.put(key, &value, None).unwrap();
    let retrieved: Option<TestData> = sync_cache.get(key).unwrap();
    assert_eq!(retrieved, Some(value));

    assert!(sync_cache.contains(key).unwrap());
    assert!(sync_cache.remove(key).unwrap());
    assert!(!sync_cache.contains(key).unwrap());
}

#[tokio::test]
async fn test_recovery_hints() {
    let temp_dir = TempDir::new().unwrap();
    let config = UnifiedCacheConfig {
        max_size_bytes: 100, // Very small limit
        ..Default::default()
    };

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(config)
        .build_async()
        .await
        .unwrap();

    let large_value = TestData::new(1, 200);
    let result = cache.put("key", &large_value, None).await;

    match result {
        Err(e) => match e.recovery_hint() {
            RecoveryHint::IncreaseCapacity { suggested_bytes } => {
                assert!(*suggested_bytes >= 200);
            }
            _ => panic!("Expected IncreaseCapacity hint"),
        },
        Ok(_) => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_cache_statistics_accuracy() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Perform known operations
    for i in 0..5 {
        let key = format!("stats-{}", i);
        let value = TestData::new(i, 50);
        cache.put(&key, &value, None).await.unwrap();
    }

    // Hits
    for i in 0..3 {
        let key = format!("stats-{}", i);
        let _: Option<TestData> = cache.get(&key).await.unwrap();
    }

    // Misses
    for i in 10..12 {
        let key = format!("stats-{}", i);
        let _: Option<TestData> = cache.get(&key).await.unwrap();
    }

    // Removals
    cache.remove("stats-0").await.unwrap();
    cache.remove("stats-1").await.unwrap();

    let stats = cache.statistics().await.unwrap();
    assert_eq!(stats.writes, 5);
    assert_eq!(stats.hits, 3);
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.removals, 2);
    assert_eq!(stats.entry_count, 3); // 5 - 2 removed
}

#[test]
fn test_cache_key_validation() {
    // Valid keys
    assert!("valid_key".validate().is_ok());
    assert!("path/to/resource".validate().is_ok());
    assert!("key-with-dashes_and_underscores".validate().is_ok());
    assert!("key.with.dots".validate().is_ok());

    // Invalid keys
    assert!("".validate().is_err());
    assert!("key\0with\0nulls".validate().is_err());

    let long_key = "x".repeat(2000);
    assert!(long_key.validate().is_err());
}
