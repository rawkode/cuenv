//! Integration tests for cache monitoring and observability

use cuenv::cache::{
    Cache, CacheConfig, CacheError, MonitoredCache, MonitoredCacheBuilder, ProductionCache,
};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_monitoring_basic_operations() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = MonitoredCacheBuilder::new(base_cache)
        .with_service_name("test-monitoring")
        .build()
        .unwrap();

    // Perform operations
    monitored.put("key1", &"value1", None).await?;
    monitored.put("key2", &"value2", None).await?;

    // Generate hits
    let _: Option<String> = monitored.get("key1").await?;
    let _: Option<String> = monitored.get("key1").await?;

    // Generate misses
    let _: Option<String> = monitored.get("nonexistent").await?;

    // Check hit rate
    let hit_rate = monitored.monitor().hit_rate();
    assert!(hit_rate > 0.0);
    assert!(hit_rate < 1.0); // We had both hits and misses

    // Check metrics text contains expected metrics
    let metrics = monitored.metrics_text();
    assert!(metrics.contains("cuenv_cache_hits_total"));
    assert!(metrics.contains("cuenv_cache_misses_total"));
    assert!(metrics.contains("cuenv_cache_writes_total"));

    Ok(())
}

#[tokio::test]
async fn test_hit_rate_analysis() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = MonitoredCacheBuilder::new(base_cache)
        .with_service_name("test-hit-rate")
        .build()
        .unwrap();

    // Generate pattern-based access
    for i in 0..10 {
        monitored
            .put(&format!("user:{}", i), &format!("data{}", i), None)
            .await?;
        monitored
            .put(&format!("session:{}", i), &format!("data{}", i), None)
            .await?;
    }

    // Access with patterns
    for i in 0..20 {
        let _: Option<String> = monitored.get(&format!("user:{}", i % 10)).await?;
        if i < 10 {
            let _: Option<String> = monitored.get(&format!("session:{}", i)).await?;
        } else {
            // These will be misses
            let _: Option<String> = monitored.get(&format!("session:{}", i)).await?;
        }
    }

    let report = monitored.hit_rate_report();

    // Check time-based windows
    assert!(report.one_minute >= 0.0 && report.one_minute <= 1.0);

    // Check pattern analysis
    let user_pattern = report.key_patterns.iter().find(|p| p.pattern == "user:*");
    assert!(user_pattern.is_some());
    assert_eq!(user_pattern.unwrap().hit_rate, 1.0); // All user keys were hits

    let session_pattern = report
        .key_patterns
        .iter()
        .find(|p| p.pattern == "session:*");
    assert!(session_pattern.is_some());
    assert!(session_pattern.unwrap().hit_rate < 1.0); // Some session keys were misses

    Ok(())
}

#[tokio::test]
async fn test_performance_profiling() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = MonitoredCacheBuilder::new(base_cache)
        .with_service_name("test-profiling")
        .with_profiling()
        .build()
        .unwrap();

    // Perform operations to generate profiling data
    for i in 0..100 {
        monitored
            .put(&format!("key{}", i), &format!("value{}", i), None)
            .await?;
        let _: Option<String> = monitored.get(&format!("key{}", i)).await?;
    }

    // Get flamegraph data
    let flamegraph = monitored.flamegraph_data();
    assert!(!flamegraph.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_concurrent_monitoring() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = Arc::new(
        MonitoredCacheBuilder::new(base_cache)
            .with_service_name("test-concurrent")
            .build()
            .unwrap(),
    );

    // Spawn multiple tasks that perform operations
    let mut handles = Vec::new();

    for task_id in 0..10 {
        let cache = Arc::clone(&monitored);
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let key = format!("task{}:key{}", task_id, i);
                cache.put(&key, &format!("value{}", i), None).await.unwrap();
                let _: Option<String> = cache.get(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Check real-time stats
    let stats = monitored.monitor().real_time_stats();
    assert_eq!(stats.operations_in_flight, 0); // All operations should be complete
    assert!(stats.avg_response_time_us > 0);

    Ok(())
}

#[tokio::test]
async fn test_error_tracking() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = MonitoredCacheBuilder::new(base_cache)
        .with_service_name("test-errors")
        .build()
        .unwrap();

    // Cause some errors
    // Invalid key
    let result: Result<Option<String>, _> = monitored.get("").await;
    assert!(result.is_err());

    // Check that errors are tracked in metrics
    let metrics = monitored.metrics_text();
    assert!(metrics.contains("cuenv_cache_errors_total"));

    Ok(())
}

#[tokio::test]
async fn test_ttl_monitoring() -> Result<(), CacheError> {
    let temp_dir = TempDir::new().unwrap();
    let base_cache = ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
        .await
        .unwrap();

    let monitored = MonitoredCacheBuilder::new(base_cache)
        .with_service_name("test-ttl")
        .build()
        .unwrap();

    // Put items with short TTL
    monitored
        .put("expires", &"soon", Some(Duration::from_millis(100)))
        .await?;

    // Should be a hit immediately
    let result: Option<String> = monitored.get("expires").await?;
    assert_eq!(result, Some("soon".to_string()));

    // Wait for expiration
    sleep(Duration::from_millis(200)).await;

    // Should be a miss after expiration
    let result: Option<String> = monitored.get("expires").await?;
    assert_eq!(result, None);

    // Check that both hit and miss were recorded
    let report = monitored.hit_rate_report();
    assert!(report.one_minute > 0.0 && report.one_minute < 1.0);

    Ok(())
}
