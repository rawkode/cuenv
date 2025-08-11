//! Advanced cache operation tests

use crate::core::Cache;
use crate::errors::Result;
use crate::traits::CacheConfig;
use tempfile::TempDir;

#[tokio::test]
async fn test_entry_limit_enforcement() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let config = CacheConfig {
        max_entries: 5, // Set small limit for testing
        ..Default::default()
    };
    let cache = Cache::new(temp_dir.path().to_path_buf(), config).await?;

    // Add entries up to the limit
    for i in 0..5 {
        match cache
            .put(&format!("key_{i}"), &format!("value_{i}"), None)
            .await
        {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
    }

    let stats = cache.statistics().await?;
    assert_eq!(stats.entry_count, 5);

    // Try to add one more entry - should fail
    match cache.put("key_6", &"value_6", None).await {
        Ok(()) => panic!("Should have failed due to entry limit"),
        Err(crate::errors::CacheError::CapacityExceeded { .. }) => {
            // Expected
        }
        Err(e) => return Err(e),
    }

    // Statistics should still show 5 entries
    let stats = cache.statistics().await?;
    assert_eq!(stats.entry_count, 5);

    Ok(())
}

#[tokio::test]
async fn test_zero_copy_mmap() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

    // Write a large value (but under the entry size limit)
    let large_data = vec![0u8; 8192]; // 8KB
    match cache.put("large", &large_data, None).await {
        Ok(()) => {}
        Err(e) => return Err(e),
    }

    // Clear memory cache to force disk read
    cache.inner.memory_cache.clear();

    // Read should use mmap
    let value: Option<Vec<u8>> = match cache.get("large").await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    assert_eq!(value, Some(large_data));

    // Check that entry in memory cache has mmap
    if let Some(entry) = cache.inner.memory_cache.get("large") {
        assert!(entry.mmap.is_some(), "Should have memory-mapped the file");
    }

    Ok(())
}

#[tokio::test]
async fn test_cache_consistency_simple() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

    let test_cases = vec![
        ("key1".to_string(), "value1".to_string()),
        ("key2".to_string(), "value2".to_string()),
        ("key3".to_string(), "value3".to_string()),
    ];

    // Put all key-value pairs
    for (key, value) in &test_cases {
        cache.put(key, value, None).await?;
    }

    // Verify all can be retrieved
    for (key, expected_value) in &test_cases {
        let actual: Option<String> = cache.get(key).await?;
        assert_eq!(actual.as_ref(), Some(expected_value));
    }

    // Clear cache
    cache.clear().await?;

    // Verify all are gone
    for (key, _) in &test_cases {
        let value: Option<String> = cache.get(key).await?;
        assert_eq!(value, None);
    }

    Ok(())
}
