//! Basic cache operation tests

use crate::core::Cache;
use crate::errors::Result;
use crate::traits::CacheConfig;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_basic_operations() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

    // Test put and get
    match cache.put("key1", &"value1", None).await {
        Ok(()) => {}
        Err(e) => return Err(e),
    }

    let value: Option<String> = match cache.get("key1").await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    assert_eq!(value, Some("value1".to_string()));

    // Test contains
    match cache.contains("key1").await {
        Ok(true) => {}
        Ok(false) => panic!("Key should exist"),
        Err(e) => return Err(e),
    }

    match cache.contains("key2").await {
        Ok(false) => {}
        Ok(true) => panic!("Key should not exist"),
        Err(e) => return Err(e),
    }

    // Test remove
    match cache.remove("key1").await {
        Ok(true) => {}
        Ok(false) => panic!("Key should have been removed"),
        Err(e) => return Err(e),
    }

    match cache.contains("key1").await {
        Ok(false) => {}
        Ok(true) => panic!("Key should not exist after removal"),
        Err(e) => return Err(e),
    }

    Ok(())
}

#[tokio::test]
async fn test_expiration() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

    // Put with short TTL
    match cache
        .put("expires", &"soon", Some(Duration::from_millis(50)))
        .await
    {
        Ok(()) => {}
        Err(e) => return Err(e),
    }

    // Should exist immediately
    match cache.contains("expires").await {
        Ok(true) => {}
        Ok(false) => panic!("Key should exist"),
        Err(e) => return Err(e),
    }

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should be expired
    let value: Option<String> = match cache.get("expires").await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    assert_eq!(value, None);

    Ok(())
}

#[tokio::test]
async fn test_statistics() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache = Cache::new(temp_dir.path().to_path_buf(), CacheConfig::default()).await?;

    match cache.put("key1", &"value1", None).await {
        Ok(()) => {}
        Err(e) => return Err(e),
    }

    let _: Option<String> = match cache.get("key1").await {
        Ok(v) => v,
        Err(e) => return Err(e),
    }; // Hit

    let _: Option<String> = match cache.get("key2").await {
        Ok(v) => v,
        Err(e) => return Err(e),
    }; // Miss

    match cache.remove("key1").await {
        Ok(_) => {}
        Err(e) => return Err(e),
    }

    let stats = match cache.statistics().await {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    assert_eq!(stats.writes, 1);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.removals, 1);

    Ok(())
}