//! Comprehensive unit tests for cache traits and supporting structures
//!
//! This test suite aims to achieve high coverage for cache traits and related
//! helper types through focused unit testing.

use cuenv::cache::{Cache, CacheMetadata, ProductionCache, UnifiedCacheConfig};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// Test configuration for cache units tests
fn test_config() -> UnifiedCacheConfig {
    UnifiedCacheConfig {
        max_size_bytes: 1024 * 1024, // 1MB
        max_entries: 100,
        default_ttl: Some(Duration::from_secs(3600)), // 1 hour
        compression_threshold: Some(512),             // Compress > 512 bytes
        cleanup_interval: Duration::from_secs(60),
        encryption_enabled: false,
        compression_enabled: true,
        compression_level: Some(3),
        compression_min_size: Some(256),
        eviction_policy: Some("lru".to_string()),
        max_memory_size: Some(512 * 1024), // 512KB memory
        max_disk_size: Some(1024 * 1024),  // 1MB disk
    }
}

/// Test data structure for serialization tests
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestData {
    id: u64,
    name: String,
    values: Vec<i32>,
    optional: Option<String>,
}

impl TestData {
    fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            values: vec![1, 2, 3, id as i32],
            optional: if id % 2 == 0 {
                Some(format!("even_{}", id))
            } else {
                None
            },
        }
    }
}

#[tokio::test]
async fn test_cache_metadata_creation_and_fields() {
    let now = SystemTime::now();
    let expires = now + Duration::from_secs(3600);

    let metadata = CacheMetadata {
        created_at: now,
        last_accessed: now,
        expires_at: Some(expires),
        size_bytes: 1024,
        access_count: 5,
        content_hash: "abc123".to_string(),
        cache_version: 1,
    };

    assert_eq!(metadata.created_at, now);
    assert_eq!(metadata.last_accessed, now);
    assert_eq!(metadata.expires_at, Some(expires));
    assert_eq!(metadata.size_bytes, 1024);
    assert_eq!(metadata.access_count, 5);
    assert_eq!(metadata.content_hash, "abc123");
    assert_eq!(metadata.cache_version, 1);

    // Test Debug trait
    let debug_str = format!("{:?}", metadata);
    assert!(debug_str.contains("CacheMetadata"));
    assert!(debug_str.contains("size_bytes: 1024"));
}

#[tokio::test]
async fn test_cache_metadata_serialization() {
    let metadata = CacheMetadata {
        created_at: UNIX_EPOCH + Duration::from_secs(1234567890),
        last_accessed: UNIX_EPOCH + Duration::from_secs(1234567891),
        expires_at: Some(UNIX_EPOCH + Duration::from_secs(1234567892)),
        size_bytes: 2048,
        access_count: 10,
        content_hash: "hash123".to_string(),
        cache_version: 2,
    };

    // Test serialization
    let serialized = serde_json::to_string(&metadata).unwrap();
    assert!(serialized.contains("\"size_bytes\":2048"));
    assert!(serialized.contains("\"access_count\":10"));
    assert!(serialized.contains("\"hash123\""));

    // Test deserialization
    let deserialized: CacheMetadata = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.size_bytes, metadata.size_bytes);
    assert_eq!(deserialized.access_count, metadata.access_count);
    assert_eq!(deserialized.content_hash, metadata.content_hash);
    assert_eq!(deserialized.cache_version, metadata.cache_version);
}

#[tokio::test]
async fn test_cache_metadata_clone() {
    let original = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: 512,
        access_count: 3,
        content_hash: "clone_test".to_string(),
        cache_version: 1,
    };

    let cloned = original.clone();

    assert_eq!(original.size_bytes, cloned.size_bytes);
    assert_eq!(original.access_count, cloned.access_count);
    assert_eq!(original.content_hash, cloned.content_hash);
    assert_eq!(original.cache_version, cloned.cache_version);
}

#[tokio::test]
async fn test_basic_cache_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test put and get with string
    let key = "test_key";
    let value = "test_value";
    cache.put(key, &value, None).await.unwrap();

    let retrieved: Option<String> = cache.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.to_string()));

    // Test put and get with complex data
    let complex_key = "complex_data";
    let complex_value = TestData::new(42, "test_item");
    cache.put(complex_key, &complex_value, None).await.unwrap();

    let retrieved_complex: Option<TestData> = cache.get(complex_key).await.unwrap();
    assert_eq!(retrieved_complex, Some(complex_value));
}

#[tokio::test]
async fn test_cache_with_ttl() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test with long TTL
    let key = "ttl_test";
    let value = "expires_later";
    let ttl = Duration::from_secs(3600); // 1 hour
    cache.put(key, &value, Some(ttl)).await.unwrap();

    let retrieved: Option<String> = cache.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.to_string()));

    // Test metadata includes expiration
    let metadata = cache.metadata(key).await.unwrap();
    assert!(metadata.is_some());
    let meta = metadata.unwrap();
    assert!(meta.expires_at.is_some());
    assert!(meta.expires_at.unwrap() > SystemTime::now());
}

#[tokio::test]
async fn test_cache_without_ttl() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    let key = "no_ttl_test";
    let value = "never_expires";
    cache.put(key, &value, None).await.unwrap();

    let metadata = cache.metadata(key).await.unwrap();
    assert!(metadata.is_some());
    let meta = metadata.unwrap();
    assert!(meta.expires_at.is_none());
}

#[tokio::test]
async fn test_cache_contains() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    let key = "contains_test";
    assert!(!cache.contains(key).await.unwrap());

    cache.put(key, &"value", None).await.unwrap();
    assert!(cache.contains(key).await.unwrap());
}

#[tokio::test]
async fn test_cache_remove() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    let key = "remove_test";
    let value = "to_be_removed";

    // Remove non-existent key
    assert!(!cache.remove(key).await.unwrap());

    // Add value and remove it
    cache.put(key, &value, None).await.unwrap();
    assert!(cache.contains(key).await.unwrap());

    let removed = cache.remove(key).await.unwrap();
    assert!(removed);
    assert!(!cache.contains(key).await.unwrap());

    // Try to remove again
    assert!(!cache.remove(key).await.unwrap());
}

#[tokio::test]
async fn test_cache_clear() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Add multiple items
    for i in 0..5 {
        let key = format!("clear_test_{}", i);
        let value = format!("value_{}", i);
        cache.put(&key, &value, None).await.unwrap();
    }

    // Verify items exist
    for i in 0..5 {
        let key = format!("clear_test_{}", i);
        assert!(cache.contains(&key).await.unwrap());
    }

    // Clear cache
    cache.clear().await.unwrap();

    // Verify items are gone
    for i in 0..5 {
        let key = format!("clear_test_{}", i);
        assert!(!cache.contains(&key).await.unwrap());
    }
}

#[tokio::test]
async fn test_cache_statistics() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    let initial_stats = cache.statistics().await.unwrap();
    assert_eq!(initial_stats.hits, 0);
    assert_eq!(initial_stats.misses, 0);
    assert_eq!(initial_stats.writes, 0);

    // Perform operations and check stats
    cache.put("stats_test", &"value", None).await.unwrap();
    let stats_after_put = cache.statistics().await.unwrap();
    assert!(stats_after_put.writes >= initial_stats.writes);

    // Multiple gets to increase hits
    let _: Option<String> = cache.get("stats_test").await.unwrap();
    let _: Option<String> = cache.get("stats_test").await.unwrap();

    let final_stats = cache.statistics().await.unwrap();
    assert!(final_stats.hits >= stats_after_put.hits);
}

#[tokio::test]
async fn test_batch_get_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Setup test data
    let test_pairs = vec![
        ("batch_1", "value_1"),
        ("batch_2", "value_2"),
        ("batch_3", "value_3"),
    ];

    for (key, value) in &test_pairs {
        cache.put(key, value, None).await.unwrap();
    }

    // Test batch get
    let keys: Vec<String> = test_pairs.iter().map(|(k, _)| k.to_string()).collect();
    let results: Vec<(String, Option<String>)> = cache.get_many(&keys).await.unwrap();

    assert_eq!(results.len(), 3);
    for (i, (key, value)) in results.iter().enumerate() {
        assert_eq!(key, test_pairs[i].0);
        assert_eq!(value.as_ref().unwrap(), test_pairs[i].1);
    }

    // Test batch get with missing keys
    let mixed_keys = vec![
        "batch_1".to_string(),
        "missing_key".to_string(),
        "batch_2".to_string(),
    ];
    let mixed_results: Vec<(String, Option<String>)> = cache.get_many(&mixed_keys).await.unwrap();

    assert_eq!(mixed_results.len(), 3);
    assert_eq!(mixed_results[0].1, Some("value_1".to_string()));
    assert_eq!(mixed_results[1].1, None);
    assert_eq!(mixed_results[2].1, Some("value_2".to_string()));
}

#[tokio::test]
async fn test_batch_put_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test batch put
    let entries = vec![
        ("batch_put_1".to_string(), "value_1", None),
        (
            "batch_put_2".to_string(),
            "value_2",
            Some(Duration::from_secs(3600)),
        ),
        ("batch_put_3".to_string(), "value_3", None),
    ];

    cache.put_many(&entries).await.unwrap();

    // Verify all entries were stored
    for (key, expected_value, _) in &entries {
        let retrieved: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(expected_value.to_string()));
    }

    // Verify TTL was set correctly
    let metadata2 = cache.metadata("batch_put_2").await.unwrap();
    assert!(metadata2.unwrap().expires_at.is_some());

    let metadata1 = cache.metadata("batch_put_1").await.unwrap();
    assert!(metadata1.unwrap().expires_at.is_none());
}

#[tokio::test]
async fn test_cache_metadata_access() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    let key = "metadata_test";
    let value = "test_value_for_metadata";

    // Non-existent key should return None
    let missing_metadata = cache.metadata(key).await.unwrap();
    assert!(missing_metadata.is_none());

    // Add value and check metadata
    cache
        .put(key, &value, Some(Duration::from_secs(1800)))
        .await
        .unwrap();

    let metadata = cache.metadata(key).await.unwrap();
    assert!(metadata.is_some());

    let meta = metadata.unwrap();
    assert!(meta.size_bytes > 0);
    assert!(!meta.content_hash.is_empty());
    assert!(meta.expires_at.is_some());
    assert_eq!(meta.access_count, 0); // Metadata access doesn't count as cache access
}

#[tokio::test]
async fn test_different_data_types() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test various data types
    cache.put("bool_key", &true, None).await.unwrap();
    cache.put("i32_key", &42i32, None).await.unwrap();
    cache.put("u64_key", &12345u64, None).await.unwrap();
    cache.put("f64_key", &3.14159f64, None).await.unwrap();
    cache
        .put("vec_key", &vec![1, 2, 3, 4, 5], None)
        .await
        .unwrap();

    // Retrieve and verify
    let bool_val: Option<bool> = cache.get("bool_key").await.unwrap();
    assert_eq!(bool_val, Some(true));

    let i32_val: Option<i32> = cache.get("i32_key").await.unwrap();
    assert_eq!(i32_val, Some(42));

    let u64_val: Option<u64> = cache.get("u64_key").await.unwrap();
    assert_eq!(u64_val, Some(12345));

    let f64_val: Option<f64> = cache.get("f64_key").await.unwrap();
    assert_eq!(f64_val, Some(3.14159));

    let vec_val: Option<Vec<i32>> = cache.get("vec_key").await.unwrap();
    assert_eq!(vec_val, Some(vec![1, 2, 3, 4, 5]));
}

#[tokio::test]
async fn test_large_value_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test value larger than compression threshold
    let large_value = "x".repeat(1000); // 1KB value
    cache.put("large_value", &large_value, None).await.unwrap();

    let retrieved: Option<String> = cache.get("large_value").await.unwrap();
    assert_eq!(retrieved, Some(large_value));

    // Check metadata
    let metadata = cache.metadata("large_value").await.unwrap();
    let meta = metadata.unwrap();
    assert!(meta.size_bytes >= 1000);
}

#[tokio::test]
async fn test_key_validation() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test valid keys
    let valid_keys = vec![
        "simple_key",
        "key_with_underscores",
        "key-with-dashes",
        "key.with.dots",
        "key123",
        "MixedCaseKey",
    ];

    for key in valid_keys {
        let result = cache.put(key, &"value", None).await;
        assert!(result.is_ok(), "Key '{}' should be valid", key);
    }
}

#[tokio::test]
async fn test_cache_config_defaults() {
    let config = UnifiedCacheConfig {
        max_size_bytes: 100,
        max_entries: 50,
        default_ttl: None,
        compression_threshold: None,
        cleanup_interval: Duration::from_secs(30),
        encryption_enabled: false,
        compression_enabled: false,
        compression_level: None,
        compression_min_size: None,
        eviction_policy: None,
        max_memory_size: None,
        max_disk_size: None,
    };

    // Test that configuration is applied correctly
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
        .await
        .unwrap();

    // Verify cache works with minimal config
    cache.put("config_test", &"value", None).await.unwrap();
    let retrieved: Option<String> = cache.get("config_test").await.unwrap();
    assert_eq!(retrieved, Some("value".to_string()));

    let stats = cache.statistics().await.unwrap();
    assert_eq!(stats.max_bytes, 100);
}

#[tokio::test]
async fn test_empty_and_special_values() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), test_config())
        .await
        .unwrap();

    // Test empty string
    cache.put("empty_string", &"", None).await.unwrap();
    let empty: Option<String> = cache.get("empty_string").await.unwrap();
    assert_eq!(empty, Some("".to_string()));

    // Test empty vector
    let empty_vec: Vec<i32> = vec![];
    cache.put("empty_vec", &empty_vec, None).await.unwrap();
    let retrieved_vec: Option<Vec<i32>> = cache.get("empty_vec").await.unwrap();
    assert_eq!(retrieved_vec, Some(vec![]));

    // Test Option values
    cache
        .put("some_value", &Some("present"), None)
        .await
        .unwrap();
    cache
        .put("none_value", &None::<String>, None)
        .await
        .unwrap();

    let some_retrieved: Option<Option<String>> = cache.get("some_value").await.unwrap();
    assert_eq!(some_retrieved, Some(Some("present".to_string())));

    let none_retrieved: Option<Option<String>> = cache.get("none_value").await.unwrap();
    assert_eq!(none_retrieved, Some(None));
}
