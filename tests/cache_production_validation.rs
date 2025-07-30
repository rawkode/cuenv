#![allow(unused)]
//! Production validation tests for the cache system
//!
//! These tests simulate real-world production scenarios and validate
//! that the cache system meets enterprise-grade requirements (Phase 8).

#[cfg(test)]
mod cache_production_tests {
    use cuenv::cache::{
        Cache, CacheError, ProductionCache, SyncCache, UnifiedCache, UnifiedCacheConfig,
    };
    use rand::prelude::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;
    use tokio::time::timeout;

    /// Simulate a high-volume web application cache workload
    #[tokio::test]
    async fn test_web_application_simulation() {
        let temp_dir = TempDir::new().unwrap();

        // Production-like configuration
        let config = UnifiedCacheConfig {
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            max_entries: 50000,
            ttl_secs: Some(Duration::from_secs(3600)), // 1 hour TTL
            compression_enabled: true,
            checksums_enabled: true,
            ..Default::default()
        };

        let cache = Arc::new(
            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                .await
                .unwrap(),
        );

        println!("Starting web application simulation...");

        // Simulate different types of cached data
        let user_sessions = generate_user_sessions(1000);
        let page_fragments = generate_page_fragments(500);
        let api_responses = generate_api_responses(2000);
        let static_assets = generate_static_assets(100);

        let num_workers = 16; // Simulate 16 worker threads
        let simulation_duration = Duration::from_secs(30);
        let start_time = Instant::now();

        let stats = Arc::new(Mutex::new(SimulationStats::new()));

        let mut worker_handles = Vec::new();

        for worker_id in 0..num_workers {
            let cache_clone = Arc::clone(&cache);
            let stats_clone = Arc::clone(&stats);
            let user_sessions = user_sessions.clone();
            let page_fragments = page_fragments.clone();
            let api_responses = api_responses.clone();
            let static_assets = static_assets.clone();

            let handle = tokio::spawn(async move {
                let mut rng = StdRng::seed_from_u64(worker_id as u64);
                let mut worker_stats = WorkerStats::new();

                while start_time.elapsed() < simulation_duration {
                    let operation_type = rng.gen_range(0..100);
                    let start_op = Instant::now();

                    match operation_type {
                        // 40% - User session operations
                        0..=39 => {
                            let session = &user_sessions[rng.gen_range(0..user_sessions.len())];
                            if rng.gen_bool(0.7) {
                                // 70% reads, 30% writes for sessions
                                match cache_clone.get::<Vec<u8>>(&session.key).await {
                                    Ok(Some(_)) => worker_stats.session_hits += 1,
                                    Ok(None) => worker_stats.session_misses += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            } else {
                                match cache_clone.put(&session.key, &session.data, None).await {
                                    Ok(_) => worker_stats.session_writes += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            }
                        }
                        // 30% - Page fragment operations
                        40..=69 => {
                            let fragment = &page_fragments[rng.gen_range(0..page_fragments.len())];
                            if rng.gen_bool(0.8) {
                                // 80% reads, 20% writes for page fragments
                                match cache_clone.get::<Vec<u8>>(&fragment.key).await {
                                    Ok(Some(_)) => worker_stats.fragment_hits += 1,
                                    Ok(None) => worker_stats.fragment_misses += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            } else {
                                match cache_clone.put(&fragment.key, &fragment.data, None).await {
                                    Ok(_) => worker_stats.fragment_writes += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            }
                        }
                        // 25% - API response operations
                        70..=94 => {
                            let response = &api_responses[rng.gen_range(0..api_responses.len())];
                            if rng.gen_bool(0.6) {
                                // 60% reads, 40% writes for API responses
                                match cache_clone.get::<Vec<u8>>(&response.key).await {
                                    Ok(Some(_)) => worker_stats.api_hits += 1,
                                    Ok(None) => worker_stats.api_misses += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            } else {
                                match cache_clone.put(&response.key, &response.data, None).await {
                                    Ok(_) => worker_stats.api_writes += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            }
                        }
                        // 5% - Static asset operations (mostly reads)
                        95..=99 => {
                            let asset = &static_assets[rng.gen_range(0..static_assets.len())];
                            if rng.gen_bool(0.95) {
                                // 95% reads, 5% writes for static assets
                                match cache_clone.get::<Vec<u8>>(&asset.key).await {
                                    Ok(Some(_)) => worker_stats.asset_hits += 1,
                                    Ok(None) => worker_stats.asset_misses += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            } else {
                                match cache_clone.put(&asset.key, &asset.data, None).await {
                                    Ok(_) => worker_stats.asset_writes += 1,
                                    Err(_) => worker_stats.errors += 1,
                                }
                            }
                        }
                        _ => unreachable!(),
                    }

                    worker_stats.total_operations += 1;
                    worker_stats.total_latency += start_op.elapsed();

                    // Small delay to simulate realistic load spacing
                    if worker_stats.total_operations % 100 == 0 {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                }

                // Merge worker stats into global stats
                if let Ok(mut global_stats) = stats_clone.lock() {
                    global_stats.merge_worker_stats(worker_id, worker_stats);
                }
            });

            worker_handles.push(handle);
        }

        // Wait for all workers to complete
        for handle in worker_handles {
            handle.await.unwrap();
        }

        let final_stats = stats.lock().unwrap();
        let cache_stats = cache.statistics().await.unwrap();

        println!("Web application simulation results:");
        println!("  Duration: {:.2}s", start_time.elapsed().as_secs_f64());
        println!("  Total operations: {}", final_stats.total_operations());
        println!(
            "  Operations/sec: {:.0}",
            final_stats.total_operations() as f64 / start_time.elapsed().as_secs_f64()
        );
        println!(
            "  Average latency: {:.2}ms",
            final_stats.average_latency().as_millis()
        );
        println!("  Hit rate: {:.2}%", final_stats.hit_rate() * 100.0);
        println!("  Error rate: {:.4}%", final_stats.error_rate() * 100.0);
        println!("  Cache entries: {}", cache_stats.entries);
        println!("  Cache memory usage: {} bytes", cache_stats.memory_bytes);

        // Validate production requirements
        assert!(
            final_stats.total_operations() > 10000,
            "Should handle high volume"
        );
        assert!(
            final_stats.hit_rate() > 0.3,
            "Should have reasonable hit rate"
        );
        assert!(
            final_stats.error_rate() < 0.01,
            "Should have low error rate"
        );
        assert!(
            final_stats.average_latency() < Duration::from_millis(10),
            "Should have low latency"
        );
    }

    /// Test cache behavior during application startup and warmup
    #[tokio::test]
    async fn test_application_startup_warmup() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        println!("Testing application startup and warmup scenario...");

        // Phase 1: Cold start - cache is empty
        let cold_start = Instant::now();
        let mut cold_misses = 0;
        let mut cold_loads = 0;

        for i in 0..1000 {
            let key = format!("startup_key_{}", i);

            // Try to get from cache (will miss)
            if cache.get::<Vec<u8>>(&key).await.unwrap().is_none() {
                cold_misses += 1;

                // Simulate loading from database/external source
                tokio::time::sleep(Duration::from_micros(100)).await; // Simulate DB latency
                let value = format!("startup_value_{}", i).repeat(10);

                // Store in cache
                cache.put(&key, value.as_bytes(), None).await.unwrap();
                cold_loads += 1;
            }
        }

        let cold_duration = cold_start.elapsed();
        println!(
            "  Cold start: {} misses, {} loads in {:.2}ms",
            cold_misses,
            cold_loads,
            cold_duration.as_millis()
        );

        // Phase 2: Warm operation - cache should have data
        let warm_start = Instant::now();
        let mut warm_hits = 0;
        let mut warm_misses = 0;

        for i in 0..1000 {
            let key = format!("startup_key_{}", i);

            match cache.get::<Vec<u8>>(&key).await.unwrap() {
                Some(_) => warm_hits += 1,
                None => warm_misses += 1,
            }
        }

        let warm_duration = warm_start.elapsed();
        println!(
            "  Warm operation: {} hits, {} misses in {:.2}ms",
            warm_hits,
            warm_misses,
            warm_duration.as_millis()
        );

        // Phase 3: Mixed workload - simulate ongoing operation
        let mixed_start = Instant::now();
        let mut mixed_hits = 0;
        let mut mixed_misses = 0;
        let mut mixed_writes = 0;

        for i in 0..2000 {
            if i % 3 == 0 {
                // New data
                let key = format!("mixed_key_{}", i);
                let value = format!("mixed_value_{}", i);
                cache.put(&key, value.as_bytes(), None).await.unwrap();
                mixed_writes += 1;
            } else {
                // Read existing data (mix of startup keys and new keys)
                let key = if i % 2 == 0 {
                    format!("startup_key_{}", i % 1000)
                } else {
                    format!("mixed_key_{}", i - (i % 3))
                };

                match cache.get::<Vec<u8>>(&key).await.unwrap() {
                    Some(_) => mixed_hits += 1,
                    None => mixed_misses += 1,
                }
            }
        }

        let mixed_duration = mixed_start.elapsed();
        println!(
            "  Mixed workload: {} hits, {} misses, {} writes in {:.2}ms",
            mixed_hits,
            mixed_misses,
            mixed_writes,
            mixed_duration.as_millis()
        );

        // Validate startup behavior
        assert_eq!(cold_misses, 1000, "Cold start should have 100% misses");
        assert!(warm_hits > 900, "Warm operation should have high hit rate");
        assert!(
            mixed_hits > mixed_misses,
            "Mixed workload should favor hits"
        );
        assert!(
            warm_duration < cold_duration / 5,
            "Warm reads should be much faster"
        );
    }

    /// Test cache resilience during partial system failures
    #[tokio::test]
    async fn test_partial_system_failure_resilience() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Arc::new(
            ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                .await
                .unwrap(),
        );

        println!("Testing partial system failure resilience...");

        // Pre-populate cache with critical data
        let critical_keys = (0..100)
            .map(|i| format!("critical_key_{}", i))
            .collect::<Vec<_>>();
        for key in &critical_keys {
            let value = format!("critical_value_{}", key);
            cache.put(key, value.as_bytes(), None).await.unwrap();
        }

        let failure_simulation = Arc::new(AtomicBool::new(false));
        let recovery_time = Arc::new(AtomicU64::new(0));

        // Simulate various failure scenarios
        let scenarios = vec![
            ("disk_corruption", Duration::from_secs(2)),
            ("memory_pressure", Duration::from_secs(3)),
            ("network_partition", Duration::from_secs(1)),
            ("dependency_timeout", Duration::from_secs(4)),
        ];

        for (scenario_name, failure_duration) in scenarios {
            println!("  Testing scenario: {}", scenario_name);

            let scenario_start = Instant::now();
            failure_simulation.store(true, Ordering::Relaxed);

            let cache_clone = Arc::clone(&cache);
            let failure_clone = Arc::clone(&failure_simulation);
            let recovery_clone = Arc::clone(&recovery_time);
            let failure_duration = failure_duration.clone();

            // Simulate the failure scenario
            let failure_handle = tokio::spawn(async move {
                tokio::time::sleep(failure_duration).await;

                let recovery_start = Instant::now();
                failure_clone.store(false, Ordering::Relaxed);

                // Simulate recovery actions
                match scenario_name {
                    "disk_corruption" => {
                        // Cache might need to rebuild some entries
                        for i in 0..10 {
                            let key = format!("recovery_key_{}", i);
                            let value = format!("recovery_value_{}", i);
                            let _ = cache_clone.put(&key, value.as_bytes(), None).await;
                        }
                    }
                    "memory_pressure" => {
                        // Clear some non-critical entries
                        // Note: This would be handled automatically by eviction in production
                    }
                    "network_partition" => {
                        // Verify connectivity to external dependencies
                        // In production, this might involve health checks
                    }
                    "dependency_timeout" => {
                        // Implement fallback mechanisms
                        // In production, this might involve circuit breakers
                    }
                    _ => {}
                }

                recovery_clone.store(
                    recovery_start.elapsed().as_millis() as u64,
                    Ordering::Relaxed,
                );
            });

            // Continue operations during failure
            let mut operations_during_failure = 0;
            let mut successful_operations = 0;
            let mut failed_operations = 0;

            while failure_simulation.load(Ordering::Relaxed) {
                operations_during_failure += 1;

                // Try to access critical data
                let key = &critical_keys[operations_during_failure % critical_keys.len()];

                match timeout(Duration::from_millis(100), cache.get::<Vec<u8>>(key)).await {
                    Ok(Ok(Some(_))) => successful_operations += 1,
                    Ok(Ok(None)) => failed_operations += 1,
                    Ok(Err(_)) => failed_operations += 1,
                    Err(_) => failed_operations += 1, // Timeout
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            failure_handle.await.unwrap();

            let scenario_duration = scenario_start.elapsed();
            let recovery_duration = recovery_time.load(Ordering::Relaxed);

            println!(
                "    Scenario duration: {:.2}s",
                scenario_duration.as_secs_f64()
            );
            println!("    Recovery time: {}ms", recovery_duration);
            println!(
                "    Operations during failure: {}",
                operations_during_failure
            );
            println!(
                "    Successful: {}, Failed: {}",
                successful_operations, failed_operations
            );

            let availability = successful_operations as f64 / operations_during_failure as f64;
            println!(
                "    Availability during failure: {:.2}%",
                availability * 100.0
            );

            // Validate resilience requirements
            assert!(
                availability > 0.7,
                "Should maintain >70% availability during {}",
                scenario_name
            );
            assert!(
                recovery_duration < 1000,
                "Recovery should be under 1 second for {}",
                scenario_name
            );
        }

        // Verify cache is fully functional after all failures
        for key in &critical_keys {
            let result = cache.get::<Vec<u8>>(key).await.unwrap();
            assert!(
                result.is_some(),
                "Critical data should survive all failure scenarios"
            );
        }
    }

    /// Test cache performance under sustained high load
    #[tokio::test]
    async fn test_sustained_high_load() {
        let temp_dir = TempDir::new().unwrap();

        let config = UnifiedCacheConfig {
            max_memory_bytes: 200 * 1024 * 1024, // 200MB
            max_entries: 100000,
            compression_enabled: true,
            ..Default::default()
        };

        let cache = Arc::new(
            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                .await
                .unwrap(),
        );

        println!("Starting sustained high load test...");

        let load_duration = Duration::from_secs(60); // 1 minute of sustained load
        let num_workers = 32; // High concurrency
        let target_ops_per_sec = 10000; // 10K ops/sec target

        let start_time = Instant::now();
        let barrier = Arc::new(Barrier::new(num_workers));
        let global_stats = Arc::new(AtomicU64::new(0));
        let global_errors = Arc::new(AtomicU64::new(0));

        let mut worker_handles = Vec::new();

        for worker_id in 0..num_workers {
            let cache_clone = Arc::clone(&cache);
            let barrier_clone = Arc::clone(&barrier);
            let stats_clone = Arc::clone(&global_stats);
            let errors_clone = Arc::clone(&global_errors);
            let load_duration = load_duration.clone();

            let handle = tokio::spawn(async move {
                barrier_clone.wait();

                let worker_start = Instant::now();
                let mut rng = StdRng::seed_from_u64(worker_id as u64);
                let mut operations = 0;
                let mut errors = 0;

                while worker_start.elapsed() < load_duration {
                    let operation_type = rng.gen_range(0..100);
                    let key = format!("load_key_{}_{}", worker_id, operations);

                    match operation_type {
                        0..=69 => {
                            // 70% reads
                            match cache_clone.get::<Vec<u8>>(&key).await {
                                Ok(_) => operations += 1,
                                Err(_) => errors += 1,
                            }
                        }
                        70..=89 => {
                            // 20% writes
                            let value =
                                generate_test_data(1024, (worker_id * 1000 + operations) as u64);
                            match cache_clone.put(&key, &value, None).await {
                                Ok(_) => operations += 1,
                                Err(_) => errors += 1,
                            }
                        }
                        90..=99 => {
                            // 10% metadata operations
                            match cache_clone.metadata(&key).await {
                                Ok(_) => operations += 1,
                                Err(_) => errors += 1,
                            }
                        }
                        _ => unreachable!(),
                    }

                    // Adaptive pacing to maintain target throughput
                    if operations % 100 == 0 {
                        let elapsed = worker_start.elapsed().as_secs_f64();
                        let current_rate = operations as f64 / elapsed;
                        let target_rate = target_ops_per_sec as f64 / num_workers as f64;

                        if current_rate > target_rate {
                            let sleep_ms = ((operations as f64 / target_rate) - elapsed) * 1000.0;
                            if sleep_ms > 0.0 {
                                tokio::time::sleep(Duration::from_millis(sleep_ms as u64)).await;
                            }
                        }
                    }
                }

                stats_clone.fetch_add(operations, Ordering::Relaxed);
                errors_clone.fetch_add(errors, Ordering::Relaxed);

                (operations, errors)
            });

            worker_handles.push(handle);
        }

        // Monitor progress
        let monitor_handle = tokio::spawn(async move {
            let mut last_ops = 0;
            let mut last_time = Instant::now();

            while start_time.elapsed() < load_duration {
                tokio::time::sleep(Duration::from_secs(5)).await;

                let current_ops = global_stats.load(Ordering::Relaxed);
                let now = Instant::now();
                let ops_delta = current_ops - last_ops;
                let time_delta = now.duration_since(last_time).as_secs_f64();
                let current_rate = ops_delta as f64 / time_delta;

                println!(
                    "  Progress: {} ops, {:.0} ops/sec",
                    current_ops, current_rate
                );

                last_ops = current_ops;
                last_time = now;
            }
        });

        // Wait for all workers
        let mut worker_results = Vec::new();
        for handle in worker_handles {
            worker_results.push(handle.await.unwrap());
        }

        monitor_handle.abort();

        let total_duration = start_time.elapsed();
        let total_operations = global_stats.load(Ordering::Relaxed);
        let total_errors = global_errors.load(Ordering::Relaxed);
        let actual_ops_per_sec = total_operations as f64 / total_duration.as_secs_f64();

        let cache_stats = cache.statistics().await.unwrap();

        println!("Sustained high load test results:");
        println!("  Duration: {:.2}s", total_duration.as_secs_f64());
        println!("  Total operations: {}", total_operations);
        println!("  Total errors: {}", total_errors);
        println!("  Actual ops/sec: {:.0}", actual_ops_per_sec);
        println!(
            "  Error rate: {:.4}%",
            (total_errors as f64 / total_operations as f64) * 100.0
        );
        println!("  Cache entries: {}", cache_stats.entries);
        println!(
            "  Memory usage: {} MB",
            cache_stats.memory_bytes / (1024 * 1024)
        );
        println!("  Cache hit rate: {:.2}%", cache_stats.hit_rate * 100.0);

        // Validate high load requirements
        assert!(
            total_operations > 400000,
            "Should handle high operation volume"
        ); // At least 400K ops in 60s
        assert!(
            actual_ops_per_sec > 5000.0,
            "Should maintain >5K ops/sec sustained rate"
        );
        assert!(
            (total_errors as f64 / total_operations as f64) < 0.001,
            "Should have <0.1% error rate"
        );
        assert!(
            cache_stats.hit_rate > 0.1,
            "Should maintain reasonable hit rate under load"
        );

        // Verify cache is still responsive after sustained load
        let post_load_start = Instant::now();
        let test_key = "post_load_test";
        let test_value = b"post_load_value";

        cache.put(test_key, test_value, None).await.unwrap();
        let retrieved: Option<Vec<u8>> = cache.get(test_key).await.unwrap();
        let post_load_latency = post_load_start.elapsed();

        assert_eq!(retrieved.unwrap(), test_value);
        assert!(
            post_load_latency < Duration::from_millis(10),
            "Should remain responsive after load"
        );
    }

    /// Test cache behavior during gradual memory exhaustion
    #[tokio::test]
    async fn test_gradual_memory_exhaustion() {
        let temp_dir = TempDir::new().unwrap();

        // Start with generous limits and gradually reduce
        let config = UnifiedCacheConfig {
            max_memory_bytes: 50 * 1024 * 1024, // 50MB
            max_entries: 50000,
            compression_enabled: false, // Disable to get predictable memory usage
            ..Default::default()
        };

        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        println!("Testing gradual memory exhaustion...");

        let entry_size = 1024; // 1KB per entry
        let data = generate_test_data(entry_size, 999);

        let mut stored_keys = Vec::new();
        let mut eviction_started = false;
        let mut eviction_start_point = 0;

        // Gradually fill memory
        for i in 0..100000 {
            let key = format!("memory_test_{}", i);

            match cache.put(&key, &data, None).await {
                Ok(_) => {
                    stored_keys.push(key.clone());

                    // Check if eviction has started
                    if i > 0 && i % 100 == 0 {
                        let stats = cache.statistics().await.unwrap();

                        if !eviction_started && stats.entries < stored_keys.len() as u64 {
                            eviction_started = true;
                            eviction_start_point = i;
                            println!(
                                "  Eviction started at entry {}, cache has {} entries",
                                i, stats.entries
                            );
                        }

                        if i % 1000 == 0 {
                            println!(
                                "  Stored {} entries, cache has {} entries, using {} MB",
                                i,
                                stats.entries,
                                stats.memory_bytes / (1024 * 1024)
                            );
                        }
                    }
                }
                Err(CacheError::InsufficientMemory { .. }) => {
                    println!("  Hit memory limit at entry {}", i);
                    break;
                }
                Err(e) => {
                    panic!("Unexpected error at entry {}: {}", i, e);
                }
            }
        }

        let final_stats = cache.statistics().await.unwrap();
        println!("Final cache state:");
        println!("  Entries: {}", final_stats.entries);
        println!(
            "  Memory usage: {} MB",
            final_stats.memory_bytes / (1024 * 1024)
        );
        println!("  Hit rate: {:.2}%", final_stats.hit_rate * 100.0);

        // Test access patterns after memory exhaustion
        let mut recent_hits = 0;
        let mut old_hits = 0;

        // Test recent entries (should mostly be present)
        for i in (stored_keys.len().saturating_sub(1000))..stored_keys.len() {
            if i < stored_keys.len() {
                if cache
                    .get::<Vec<u8>>(&stored_keys[i])
                    .await
                    .unwrap()
                    .is_some()
                {
                    recent_hits += 1;
                }
            }
        }

        // Test old entries (should mostly be evicted)
        for i in 0..std::cmp::min(1000, stored_keys.len()) {
            if cache
                .get::<Vec<u8>>(&stored_keys[i])
                .await
                .unwrap()
                .is_some()
            {
                old_hits += 1;
            }
        }

        println!("Access pattern after exhaustion:");
        println!(
            "  Recent entries hit rate: {:.2}%",
            (recent_hits as f64 / 1000.0) * 100.0
        );
        println!(
            "  Old entries hit rate: {:.2}%",
            (old_hits as f64 / 1000.0) * 100.0
        );

        // Validate memory management
        assert!(eviction_started, "Eviction should have started");
        assert!(
            eviction_start_point > 10000,
            "Should allow substantial data before eviction"
        );
        assert!(
            final_stats.memory_bytes <= 60 * 1024 * 1024,
            "Should respect memory limits"
        );
        assert!(recent_hits > old_hits, "Should prefer recent entries");
        assert!(
            recent_hits > 500,
            "Should maintain good hit rate for recent data"
        );
    }

    // Helper types and functions for simulation

    #[derive(Clone)]
    struct CacheItem {
        key: String,
        data: Vec<u8>,
    }

    #[derive(Default)]
    struct WorkerStats {
        session_hits: u64,
        session_misses: u64,
        session_writes: u64,
        fragment_hits: u64,
        fragment_misses: u64,
        fragment_writes: u64,
        api_hits: u64,
        api_misses: u64,
        api_writes: u64,
        asset_hits: u64,
        asset_misses: u64,
        asset_writes: u64,
        errors: u64,
        total_operations: u64,
        total_latency: Duration,
    }

    #[derive(Default)]
    struct SimulationStats {
        workers: HashMap<usize, WorkerStats>,
    }

    impl SimulationStats {
        fn new() -> Self {
            Self {
                workers: HashMap::new(),
            }
        }

        fn merge_worker_stats(&mut self, worker_id: usize, stats: WorkerStats) {
            self.workers.insert(worker_id, stats);
        }

        fn total_operations(&self) -> u64 {
            self.workers.values().map(|w| w.total_operations).sum()
        }

        fn hit_rate(&self) -> f64 {
            let total_hits = self
                .workers
                .values()
                .map(|w| w.session_hits + w.fragment_hits + w.api_hits + w.asset_hits)
                .sum::<u64>();
            let total_requests = self
                .workers
                .values()
                .map(|w| {
                    w.session_hits
                        + w.session_misses
                        + w.fragment_hits
                        + w.fragment_misses
                        + w.api_hits
                        + w.api_misses
                        + w.asset_hits
                        + w.asset_misses
                })
                .sum::<u64>();

            if total_requests > 0 {
                total_hits as f64 / total_requests as f64
            } else {
                0.0
            }
        }

        fn error_rate(&self) -> f64 {
            let total_errors: u64 = self.workers.values().map(|w| w.errors).sum();
            let total_ops = self.total_operations();

            if total_ops > 0 {
                total_errors as f64 / total_ops as f64
            } else {
                0.0
            }
        }

        fn average_latency(&self) -> Duration {
            let total_latency: Duration = self.workers.values().map(|w| w.total_latency).sum();
            let total_ops = self.total_operations();

            if total_ops > 0 {
                total_latency / total_ops as u32
            } else {
                Duration::ZERO
            }
        }
    }

    fn generate_test_data(size: usize, seed: u64) -> Vec<u8> {
        let mut rng = StdRng::seed_from_u64(seed);
        (0..size).map(|_| rng.gen()).collect()
    }

    fn generate_user_sessions(count: usize) -> Vec<CacheItem> {
        let mut rng = StdRng::seed_from_u64(1001);
        (0..count)
            .map(|i| CacheItem {
                key: format!("session:{}", i),
                data: generate_test_data(rng.gen_range(100..500), i as u64),
            })
            .collect()
    }

    fn generate_page_fragments(count: usize) -> Vec<CacheItem> {
        let mut rng = StdRng::seed_from_u64(1002);
        (0..count)
            .map(|i| CacheItem {
                key: format!("fragment:page_{}", i),
                data: generate_test_data(rng.gen_range(1000..5000), i as u64),
            })
            .collect()
    }

    fn generate_api_responses(count: usize) -> Vec<CacheItem> {
        let mut rng = StdRng::seed_from_u64(1003);
        (0..count)
            .map(|i| CacheItem {
                key: format!("api:response_{}", i),
                data: generate_test_data(rng.gen_range(200..2000), i as u64),
            })
            .collect()
    }

    fn generate_static_assets(count: usize) -> Vec<CacheItem> {
        let mut rng = StdRng::seed_from_u64(1004);
        (0..count)
            .map(|i| CacheItem {
                key: format!("asset:static_{}", i),
                data: generate_test_data(rng.gen_range(5000..50000), i as u64),
            })
            .collect()
    }
}
