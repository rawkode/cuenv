//! Integration tests for Phase 4: Eviction & Memory Management

use cuenv::cache::{Cache, CacheBuilder};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_lru_eviction_policy() {
    let temp_dir = TempDir::new().unwrap();

    // Create cache with small memory limit to trigger eviction
    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(10 * 1024), // 10KB limit
            eviction_policy: Some("lru".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Fill cache beyond limit
    for i in 0..20 {
        let key = format!("key_{i}");
        let value = vec![0u8; 1024]; // 1KB each
        cache.put(&key, &value, None).await.unwrap();

        // Access some keys to make them "recently used"
        if i % 5 == 0 {
            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        }
    }

    // Least recently used keys should be evicted
    let result: Option<Vec<u8>> = cache.get("key_1").await.unwrap();
    assert!(result.is_none(), "LRU key should have been evicted");

    // Recently accessed keys should still be present
    let result: Option<Vec<u8>> = cache.get("key_15").await.unwrap();
    assert!(result.is_some(), "Recently used key should still be cached");
}

#[tokio::test]
async fn test_lfu_eviction_policy() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(10 * 1024), // 10KB limit
            eviction_policy: Some("lfu".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Add entries with different access frequencies
    for i in 0..10 {
        let key = format!("key_{i}");
        let value = vec![0u8; 1024]; // 1KB each
        cache.put(&key, &value, None).await.unwrap();

        // Access some keys more frequently
        for _ in 0..i {
            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        }
    }

    // Add more entries to trigger eviction
    for i in 10..20 {
        let key = format!("key_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();
    }

    // Least frequently used keys should be evicted
    let result: Option<Vec<u8>> = cache.get("key_0").await.unwrap();
    assert!(result.is_none(), "LFU key should have been evicted");

    // Frequently accessed keys should still be present
    let result: Option<Vec<u8>> = cache.get("key_9").await.unwrap();
    assert!(
        result.is_some(),
        "Frequently used key should still be cached"
    );
}

#[tokio::test]
async fn test_arc_eviction_policy() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(20 * 1024), // 20KB limit
            eviction_policy: Some("arc".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Simulate mixed access pattern
    // First, fill with sequential access (T1 candidates)
    for i in 0..10 {
        let key = format!("seq_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();
    }

    // Then add frequently accessed items (T2 candidates)
    for i in 0..5 {
        let key = format!("freq_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();

        // Access multiple times to move to T2
        for _ in 0..3 {
            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        }
    }

    // Trigger eviction with new entries
    for i in 10..25 {
        let key = format!("new_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();
    }

    // Frequently accessed items should be retained
    for i in 0..5 {
        let key = format!("freq_{i}");
        let result: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        assert!(
            result.is_some(),
            "Frequently accessed key {key} should be retained"
        );
    }
}

#[tokio::test]
async fn test_memory_pressure_handling() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(50 * 1024), // 50KB limit
            max_disk_size: Some(100 * 1024),  // 100KB disk limit
            eviction_policy: Some("lru".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Fill cache to create memory pressure
    let mut keys = Vec::new();
    for i in 0..100 {
        let key = format!("pressure_test_{i}");
        let value = vec![0u8; 1024]; // 1KB each

        match cache.put(&key, &value, None).await {
            Ok(()) => keys.push(key),
            Err(e) => {
                // Should eventually hit quota and trigger eviction
                println!("Put failed as expected: {e}");
                break;
            }
        }
    }

    // Cache should have evicted entries to stay within limits
    let stats = cache.statistics().await.unwrap();
    assert!(
        stats.total_bytes <= 50 * 1024,
        "Cache should respect memory limit"
    );

    // Some entries should have been evicted
    let mut evicted_count = 0;
    for key in &keys[..20] {
        // Check first 20 keys
        let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
        if result.is_none() {
            evicted_count += 1;
        }
    }
    assert!(evicted_count > 0, "Some entries should have been evicted");
}

#[tokio::test]
async fn test_disk_quota_management() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(10 * 1024), // 10KB memory
            max_disk_size: Some(50 * 1024),   // 50KB disk limit
            eviction_policy: Some("lru".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Try to exceed disk quota
    let large_value = vec![0u8; 10 * 1024]; // 10KB each

    let mut successful_puts = 0;
    for i in 0..10 {
        let key = format!("disk_quota_{i}");
        match cache.put(&key, &large_value, None).await {
            Ok(()) => successful_puts += 1,
            Err(e) => {
                println!("Expected disk quota error: {e}");
                break;
            }
        }
    }

    // Should not be able to store all 10 entries (100KB) with 50KB limit
    assert!(
        successful_puts < 10,
        "Disk quota should prevent storing all entries"
    );
    assert!(
        successful_puts >= 4,
        "Should store at least 4 entries before hitting quota"
    );
}

#[tokio::test]
#[cfg_attr(coverage, ignore)]
async fn test_concurrent_eviction_safety() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(20 * 1024), // 20KB limit
            eviction_policy: Some("lru".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Spawn multiple tasks to stress test concurrent access during eviction
    let mut handles = Vec::new();

    for task_id in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..50 {
                let key = format!("concurrent_{task_id}_{i}");
                let value = vec![task_id as u8; 512]; // 512B each

                // Randomly put or get
                if i % 3 == 0 {
                    let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                } else {
                    let _ = cache.put(&key, &value, None).await;
                }

                // Small delay to increase contention
                if i % 10 == 0 {
                    sleep(Duration::from_micros(100)).await;
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify cache is still functional and within limits
    let stats = cache.statistics().await.unwrap();
    assert!(
        stats.total_bytes <= 20 * 1024,
        "Cache should respect memory limit even under concurrent load"
    );

    // Verify we can still use the cache
    cache.put("final_test", &vec![1, 2, 3], None).await.unwrap();
    let result: Option<Vec<u8>> = cache.get("final_test").await.unwrap();
    assert_eq!(result, Some(vec![1, 2, 3]));
}

#[tokio::test]
async fn test_eviction_with_ttl() {
    let temp_dir = TempDir::new().unwrap();

    let cache = CacheBuilder::new(temp_dir.path())
        .with_config(cuenv::cache::UnifiedCacheConfig {
            max_memory_size: Some(10 * 1024),
            eviction_policy: Some("lru".to_string()),
            ..Default::default()
        })
        .build_async()
        .await
        .unwrap();

    // Add entries with short TTL
    for i in 0..5 {
        let key = format!("ttl_{i}");
        let value = vec![0u8; 1024];
        cache
            .put(&key, &value, Some(Duration::from_millis(100)))
            .await
            .unwrap();
    }

    // Add entries without TTL
    for i in 5..10 {
        let key = format!("permanent_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();
    }

    // Wait for TTL entries to expire
    sleep(Duration::from_millis(200)).await;

    // Add more entries to trigger eviction
    for i in 10..15 {
        let key = format!("new_{i}");
        let value = vec![0u8; 1024];
        cache.put(&key, &value, None).await.unwrap();
    }

    // TTL entries should be gone
    for i in 0..5 {
        let key = format!("ttl_{i}");
        let result: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        assert!(result.is_none(), "Expired entry should be removed");
    }

    // Permanent entries should still exist (unless evicted)
    let mut permanent_count = 0;
    for i in 5..10 {
        let key = format!("permanent_{i}");
        let result: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        if result.is_some() {
            permanent_count += 1;
        }
    }
    assert!(permanent_count > 0, "Some permanent entries should remain");
}
