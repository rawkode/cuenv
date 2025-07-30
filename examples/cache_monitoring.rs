//! Example demonstrating cache monitoring and observability features
//!
//! This example shows how to:
//! - Create a monitored cache with full observability
//! - Start a metrics server for Prometheus scraping
//! - Generate load to demonstrate monitoring capabilities
//! - Access various monitoring endpoints

use cuenv::cache::{
    CacheConfig as UnifiedCacheConfig, CacheError, MonitoredCacheBuilder, MetricsEndpoint,
    ProductionCache, Cache,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("cuenv=debug")
        .init();

    info!("Starting cache monitoring example");

    // Create a production cache
    let cache_dir = std::env::temp_dir().join("cuenv-monitoring-example");
    let config = UnifiedCacheConfig {
        max_size_bytes: 100 * 1024 * 1024, // 100MB
        max_entries: 10000,
        ttl: Some(Duration::from_secs(3600)),
        cleanup_interval: Duration::from_secs(60),
        ..Default::default()
    };

    let base_cache = match ProductionCache::new(cache_dir, config).await {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to create cache: {}", e);
            return Err(e.into());
        }
    };

    // Create a monitored cache with profiling enabled
    let monitored_cache = match MonitoredCacheBuilder::new(base_cache)
        .with_service_name("cuenv-example")
        .with_profiling()
        .build()
    {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to create monitored cache: {}", e);
            return Err(e.into());
        }
    };

    // Create metrics endpoint
    let metrics_endpoint = MetricsEndpoint::new(monitored_cache.clone());

    info!("Cache monitoring example with metrics endpoint");
    info!("Metrics can be accessed programmatically:");

    // Generate some load to demonstrate monitoring
    info!("Generating cache load...");
    
    // Simulate different access patterns
    let load_handle = tokio::spawn(async move {
        // Pattern 1: Sequential writes and reads
        info!("Pattern 1: Sequential access");
        for i in 0..100 {
            let key = format!("sequential:{}", i);
            let value = format!("value-{}", i);
            
            if let Err(e) = monitored_cache.put(&key, &value, None).await {
                warn!("Failed to put {}: {}", key, e);
            }
            
            // Read back with 80% probability (simulate cache hits)
            if fastrand::u8(0..100) < 80 {
                let _: Option<String> = monitored_cache.get(&key).await.ok().flatten();
            }
        }

        // Pattern 2: Hot keys (frequent access to same keys)
        info!("Pattern 2: Hot keys");
        let hot_keys = vec!["hot:popular", "hot:trending", "hot:featured"];
        for _ in 0..200 {
            let key = hot_keys[fastrand::usize(0..hot_keys.len())];
            
            // First access might be a miss, subsequent ones should be hits
            let _: Option<String> = monitored_cache.get(key).await.ok().flatten();
            
            // Occasionally update hot keys
            if fastrand::u8(0..100) < 10 {
                let _ = monitored_cache.put(key, &"updated-value", None).await;
            }
        }

        // Pattern 3: Random access (lower hit rate)
        info!("Pattern 3: Random access");
        for _ in 0..100 {
            let key = format!("random:{}", fastrand::u32(0..1000));
            
            // Try to get first (likely miss)
            let result: Option<String> = monitored_cache.get(&key).await.ok().flatten();
            
            // If miss, write the value
            if result.is_none() {
                let _ = monitored_cache.put(&key, &"random-value", None).await;
            }
        }

        // Pattern 4: Batch operations
        info!("Pattern 4: Batch operations");
        let batch_prefix = "batch";
        
        // Write batch
        for i in 0..50 {
            let key = format!("{}:{}", batch_prefix, i);
            let _ = monitored_cache.put(&key, &format!("batch-value-{}", i), None).await;
        }
        
        // Read batch multiple times
        for _ in 0..3 {
            for i in 0..50 {
                let key = format!("{}:{}", batch_prefix, i);
                let _: Option<String> = monitored_cache.get(&key).await.ok().flatten();
            }
            sleep(Duration::from_millis(100)).await;
        }

        // Pattern 5: TTL testing
        info!("Pattern 5: TTL expiration");
        for i in 0..10 {
            let key = format!("ttl:{}", i);
            let ttl = Duration::from_secs(i + 1);
            let _ = monitored_cache.put(&key, &"expires-soon", Some(ttl)).await;
        }

        info!("Load generation complete!");
        
        // Print final statistics
        if let Ok(stats) = monitored_cache.statistics().await {
            info!("Final cache statistics:");
            info!("  Hits: {}", stats.hits);
            info!("  Misses: {}", stats.misses);
            info!("  Hit rate: {:.2}%", (stats.hits as f64 / (stats.hits + stats.misses) as f64) * 100.0);
            info!("  Writes: {}", stats.writes);
            info!("  Entries: {}", stats.entry_count);
            info!("  Size: {} bytes", stats.total_bytes);
        }

        // Get detailed hit rate report
        let report = monitored_cache.hit_rate_report();
        info!("Hit rate analysis:");
        info!("  1 minute: {:.2}%", report.one_minute * 100.0);
        info!("  5 minutes: {:.2}%", report.five_minutes * 100.0);
        
        if !report.key_patterns.is_empty() {
            info!("Top key patterns:");
            for (i, pattern) in report.key_patterns.iter().take(5).enumerate() {
                info!("  {}. {} - {:.2}% hit rate ({} accesses)",
                    i + 1,
                    pattern.pattern,
                    pattern.hit_rate * 100.0,
                    pattern.total_accesses
                );
            }
        }

        if !report.operation_types.is_empty() {
            info!("Operation statistics:");
            for op in &report.operation_types {
                info!("  {} - {:.2}% hit rate ({} calls)",
                    op.operation,
                    op.hit_rate * 100.0,
                    op.total_calls
                );
            }
        }
    });

    // Wait for load generation to complete
    load_handle.await?;

    info!("\nCache monitoring example complete!");
    
    // Demonstrate metrics access
    info!("\n=== Prometheus Metrics ===");
    let prometheus_metrics = metrics_endpoint.prometheus_metrics();
    println!("Metrics excerpt:\n{}", 
        prometheus_metrics.lines().take(10).collect::<Vec<_>>().join("\n"));
    
    info!("\n=== Hit Rate Analysis ===");
    if let Ok(hit_rate_json) = metrics_endpoint.hit_rate_json() {
        println!("Hit rate data:\n{}", hit_rate_json);
    }
    
    info!("\n=== Real-time Stats ===");
    if let Ok(stats_json) = metrics_endpoint.stats_json() {
        println!("Stats data:\n{}", stats_json);
    }
    
    info!("\n=== Flamegraph Data ===");
    let flamegraph = metrics_endpoint.flamegraph_data();
    if !flamegraph.is_empty() {
        let lines: Vec<&str> = flamegraph.lines().take(5).collect();
        println!("Flamegraph sample:\n{}", lines.join("\n"));
    } else {
        println!("No flamegraph data available (profiling may not be active)");
    }
    
    info!("\nMonitoring data successfully collected and displayed!");

    Ok(())
}