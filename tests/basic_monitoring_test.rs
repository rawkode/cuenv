//! Basic monitoring functionality test

use cuenv::cache::CacheMonitor;
use std::time::Duration;

#[test]
fn test_cache_monitor_creation() {
    let monitor = CacheMonitor::new("test-service").expect("Should create monitor");

    // Test basic functionality
    monitor.record_hit("test-key", "get", Duration::from_millis(10));
    monitor.record_miss("missing-key", "get", Duration::from_millis(5));
    monitor.record_write("new-key", 1024, Duration::from_millis(20));

    // Test metrics text generation
    let metrics_text = monitor.metrics_text();
    assert!(metrics_text.contains("cuenv_cache_operations_total"));

    // Test hit rate
    let hit_rate = monitor.hit_rate();
    assert!((0.0..=1.0).contains(&hit_rate));

    // Test hit rate report
    let report = monitor.hit_rate_report();
    assert!(report.one_minute >= 0.0);

    // Test real-time stats
    let stats = monitor.real_time_stats();
    assert_eq!(stats.operations_in_flight, 0);
}

#[test]
fn test_tracing_operation() {
    let monitor = CacheMonitor::new("test-tracing").expect("Should create monitor");

    let operation = monitor.start_operation("test_op", "test_key");
    operation.complete();

    // Should work without errors
}

#[test]
fn test_hit_rate_analysis() {
    let monitor = CacheMonitor::new("test-hit-rate").expect("Should create monitor");

    // Generate some patterns
    for i in 0..10 {
        monitor.record_hit(&format!("user:{i}"), "get", Duration::from_millis(1));
        monitor.record_hit(&format!("session:{i}"), "get", Duration::from_millis(1));
    }

    for i in 0..5 {
        monitor.record_miss(&format!("user:{}", i + 10), "get", Duration::from_millis(1));
    }

    let report = monitor.hit_rate_report();

    // Should have some key patterns
    assert!(!report.key_patterns.is_empty());

    // Should have operation stats
    assert!(!report.operation_types.is_empty());

    // Check that the user pattern has good hit rate
    let user_pattern = report.key_patterns.iter().find(|p| p.pattern == "user:*");
    assert!(user_pattern.is_some());
}

#[test]
fn test_prometheus_metrics_format() {
    let monitor = CacheMonitor::new("test-prometheus").expect("Should create monitor");

    // Generate some metrics
    monitor.record_hit("key1", "get", Duration::from_millis(5));
    monitor.record_miss("key2", "get", Duration::from_millis(3));
    monitor.record_write("key3", 1024, Duration::from_millis(10));

    // Need to record stats to populate cache_stats gauges
    use cuenv::cache::UnifiedCacheStatistics;
    use std::time::SystemTime;
    let stats = UnifiedCacheStatistics {
        hits: 1,
        misses: 1,
        writes: 1,
        removals: 0,
        errors: 0,
        entry_count: 1,
        total_bytes: 1024,
        max_bytes: 0,
        expired_cleanups: 0,
        stats_since: SystemTime::now(),
        compression_enabled: false,
        compression_ratio: 0.0,
        wal_recoveries: 0,
        checksum_failures: 0,
    };
    monitor.update_statistics(&stats, 512, 512);

    let metrics = monitor.metrics_text();

    // Check for expected Prometheus format
    assert!(metrics.contains("# HELP"));
    assert!(metrics.contains("# TYPE"));
    assert!(metrics.contains("cuenv_cache_operations_total"));
    assert!(metrics.contains("cuenv_cache_operation_duration_seconds"));
    assert!(metrics.contains("cuenv_cache_stats"));

    // Check that we have actual metric values with correct label syntax
    assert!(metrics.contains("operation=\"get\"") && metrics.contains("result=\"hit\""));
    assert!(metrics.contains("operation=\"get\"") && metrics.contains("result=\"miss\""));
    assert!(metrics.contains("operation=\"write\"") && metrics.contains("result=\"success\""));
}
