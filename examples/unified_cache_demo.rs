//! Example demonstrating the new unified cache API

use cuenv::cache::{Cache, CacheBuilder, CacheError, RecoveryHint, UnifiedCacheConfig};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TaskResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create cache directory
    let cache_dir = std::env::temp_dir().join("cuenv_cache_demo");

    // Build an async cache with custom configuration
    let config = UnifiedCacheConfig::default();

    let cache = CacheBuilder::new(&cache_dir)
        .with_config(config)
        .build_async()
        .await?;

    println!("Cache created at: {:?}", cache_dir);

    // Example 1: Basic put/get operations
    println!("\n=== Basic Operations ===");

    let task_result = TaskResult {
        exit_code: 0,
        stdout: "Task completed successfully".to_string(),
        stderr: String::new(),
        duration_ms: 42,
    };

    let key = "task:build:12345";

    // Store value with custom TTL
    cache
        .put(key, &task_result, Some(Duration::from_secs(60)))
        .await?;
    println!("Stored task result for key: {}", key);

    // Retrieve value
    match cache.get::<TaskResult>(key).await? {
        Some(result) => {
            println!("Retrieved: {:?}", result);
            assert_eq!(result, task_result);
        }
        None => println!("Key not found"),
    }

    // Example 2: Error handling with recovery
    println!("\n=== Error Handling ===");

    // Simulate an error scenario
    let invalid_key = "";
    match cache.get::<TaskResult>(invalid_key).await {
        Ok(_) => println!("Unexpected success"),
        Err(e) => {
            println!("Error: {}", e);
            if let RecoveryHint::Manual { instructions } = e.recovery_hint() {
                println!("Recovery: {}", instructions);
            }
        }
    }

    // Example 3: Batch operations
    println!("\n=== Batch Operations ===");

    let entries = vec![
        ("batch:1".to_string(), task_result.clone(), None),
        (
            "batch:2".to_string(),
            task_result.clone(),
            Some(Duration::from_secs(30)),
        ),
        (
            "batch:3".to_string(),
            task_result.clone(),
            Some(Duration::from_secs(90)),
        ),
    ];

    cache.put_many(&entries).await?;
    println!("Stored {} entries", entries.len());

    let keys = vec![
        "batch:1".to_string(),
        "batch:2".to_string(),
        "batch:missing".to_string(),
    ];
    let results = cache.get_many::<TaskResult>(&keys).await?;

    for (key, value) in results {
        match value {
            Some(_) => println!("Found: {}", key),
            None => println!("Missing: {}", key),
        }
    }

    // Example 4: Cache statistics
    println!("\n=== Cache Statistics ===");

    let stats = cache.statistics().await?;
    println!("Cache stats:");
    println!("  Hits: {}", stats.hits);
    println!("  Misses: {}", stats.misses);
    println!("  Writes: {}", stats.writes);
    println!("  Total size: {} bytes", stats.total_bytes);
    println!("  Entry count: {}", stats.entry_count);

    // Example 5: Metadata inspection
    println!("\n=== Metadata ===");

    if let Some(metadata) = cache.metadata(key).await? {
        println!("Metadata for key '{}':", key);
        println!("  Created: {:?}", metadata.created_at);
        println!("  Size: {} bytes", metadata.size_bytes);
        println!("  Hash: {}", metadata.content_hash);
        if let Some(expires) = metadata.expires_at {
            println!("  Expires: {:?}", expires);
        }
    }

    // Example 6: Sync cache usage
    println!("\n=== Sync Cache ===");

    // Note: Cannot create sync cache from within tokio::main
    // In real code, you would use build_sync() from non-async context
    println!("(Sync cache example skipped - would be used from non-async context)");

    // Cleanup
    println!("\n=== Cleanup ===");
    cache.clear().await?;
    println!("Cache cleared");

    Ok(())
}

// Example of using cache in production code
#[allow(dead_code)]
async fn cached_task_execution(
    cache: &cuenv::cache::ProductionCache,
    task_name: &str,
) -> Result<TaskResult, CacheError> {
    let cache_key = format!("task:{}:{}", task_name, chrono::Utc::now().timestamp());

    // Check cache first
    if let Some(cached) = cache.get::<TaskResult>(&cache_key).await? {
        tracing::info!("Cache hit for task {}", task_name);
        return Ok(cached);
    }

    // Execute task (simulated)
    tracing::info!("Cache miss for task {}, executing...", task_name);
    let result = TaskResult {
        exit_code: 0,
        stdout: format!("Output from {}", task_name),
        stderr: String::new(),
        duration_ms: 100,
    };

    // Store in cache with appropriate TTL
    let ttl = if result.exit_code == 0 {
        Some(Duration::from_secs(3600)) // 1 hour for success
    } else {
        Some(Duration::from_secs(300)) // 5 minutes for failure
    };

    cache.put(&cache_key, &result, ttl).await?;

    Ok(result)
}
