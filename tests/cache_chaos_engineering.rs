#![allow(unused)]
//! Advanced chaos engineering tests for the cache system
//!
//! This module implements sophisticated fault injection and chaos testing
//! to verify system resilience under adverse conditions (Phase 8).

#[cfg(test)]
mod cache_chaos_tests {
    use cuenv::cache::{Cache, CacheError, ProductionCache, UnifiedCache, UnifiedCacheConfig};
    use rand::prelude::*;
    use std::collections::HashMap;
    use std::fs;
    use std::io::{self, Error as IoError, ErrorKind};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;
    use tokio::time::timeout;

    /// Chaos filesystem that randomly injects failures
    struct ChaosFilesystem {
        base_path: PathBuf,
        failure_rate: f64,
        corruption_rate: f64,
        latency_injection_rate: f64,
        max_latency_ms: u64,
        failures_injected: Arc<AtomicU64>,
        corruptions_injected: Arc<AtomicU64>,
        latency_injections: Arc<AtomicU64>,
        enabled: Arc<AtomicBool>,
    }

    impl ChaosFilesystem {
        fn new(base_path: PathBuf, failure_rate: f64, corruption_rate: f64) -> Self {
            Self {
                base_path,
                failure_rate,
                corruption_rate,
                latency_injection_rate: 0.1,
                max_latency_ms: 1000,
                failures_injected: Arc::new(AtomicU64::new(0)),
                corruptions_injected: Arc::new(AtomicU64::new(0)),
                latency_injections: Arc::new(AtomicU64::new(0)),
                enabled: Arc::new(AtomicBool::new(true)),
            }
        }

        fn maybe_inject_failure(&self, operation: &str) -> Result<(), IoError> {
            if !self.enabled.load(Ordering::Relaxed) {
                return Ok(());
            }

            let mut rng = thread_rng();

            // Inject latency
            if rng.gen::<f64>() < self.latency_injection_rate {
                let latency_ms = rng.gen_range(10..=self.max_latency_ms);
                thread::sleep(Duration::from_millis(latency_ms));
                self.latency_injections.fetch_add(1, Ordering::Relaxed);
            }

            // Inject failures
            if rng.gen::<f64>() < self.failure_rate {
                self.failures_injected.fetch_add(1, Ordering::Relaxed);

                let error_kind = match rng.gen_range(0..6) {
                    0 => ErrorKind::PermissionDenied,
                    1 => ErrorKind::NotFound,
                    2 => ErrorKind::AlreadyExists,
                    3 => ErrorKind::WriteZero,
                    4 => ErrorKind::UnexpectedEof,
                    _ => ErrorKind::Other,
                };

                return Err(IoError::new(
                    error_kind,
                    format!("Chaos injection during {}", operation),
                ));
            }

            Ok(())
        }

        fn maybe_corrupt_data(&self, data: &mut [u8]) {
            if !self.enabled.load(Ordering::Relaxed) {
                return;
            }

            let mut rng = thread_rng();
            if rng.gen::<f64>() < self.corruption_rate && !data.is_empty() {
                // Corrupt a random byte
                let index = rng.gen_range(0..data.len());
                data[index] = rng.gen();
                self.corruptions_injected.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn chaos_write(&self, path: &Path, mut data: Vec<u8>) -> Result<(), IoError> {
            self.maybe_inject_failure("write")?;
            self.maybe_corrupt_data(&mut data);

            // Occasionally truncate writes
            let mut rng = thread_rng();
            if rng.gen::<f64>() < 0.05 && data.len() > 10 {
                let truncate_at = rng.gen_range(1..data.len());
                data.truncate(truncate_at);
            }

            fs::write(path, data)
        }

        fn chaos_read(&self, path: &Path) -> Result<Vec<u8>, IoError> {
            self.maybe_inject_failure("read")?;

            let mut data = fs::read(path)?;
            self.maybe_corrupt_data(&mut data);

            Ok(data)
        }

        fn enable(&self) {
            self.enabled.store(true, Ordering::Relaxed);
        }

        fn disable(&self) {
            self.enabled.store(false, Ordering::Relaxed);
        }

        fn get_stats(&self) -> (u64, u64, u64) {
            (
                self.failures_injected.load(Ordering::Relaxed),
                self.corruptions_injected.load(Ordering::Relaxed),
                self.latency_injections.load(Ordering::Relaxed),
            )
        }
    }

    /// Memory pressure simulator
    struct MemoryPressureSimulator {
        allocations: Arc<Mutex<Vec<Vec<u8>>>>,
        max_memory_mb: usize,
        enabled: Arc<AtomicBool>,
    }

    impl MemoryPressureSimulator {
        fn new(max_memory_mb: usize) -> Self {
            Self {
                allocations: Arc::new(Mutex::new(Vec::new())),
                max_memory_mb,
                enabled: Arc::new(AtomicBool::new(false)),
            }
        }

        fn start_pressure(&self) {
            self.enabled.store(true, Ordering::Relaxed);

            let allocations = Arc::clone(&self.allocations);
            let enabled = Arc::clone(&self.enabled);
            let target_mb = self.max_memory_mb;

            thread::spawn(move || {
                let mut current_mb = 0;
                let mut rng = thread_rng();

                while enabled.load(Ordering::Relaxed) {
                    if current_mb < target_mb {
                        let chunk_size = rng.gen_range(1..=10) * 1024 * 1024; // 1-10MB chunks
                        if current_mb + chunk_size / (1024 * 1024) <= target_mb {
                            let chunk = vec![rng.gen::<u8>(); chunk_size];
                            if let Ok(mut allocs) = allocations.lock() {
                                allocs.push(chunk);
                                current_mb += chunk_size / (1024 * 1024);
                            }
                        }
                    }

                    thread::sleep(Duration::from_millis(100));
                }
            });
        }

        fn stop_pressure(&self) {
            self.enabled.store(false, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(200)); // Allow cleanup

            if let Ok(mut allocs) = self.allocations.lock() {
                allocs.clear();
            }
        }
    }

    /// Network partition simulator
    struct NetworkPartitionSimulator {
        partition_active: Arc<AtomicBool>,
        packet_loss_rate: Arc<Mutex<f64>>,
    }

    impl NetworkPartitionSimulator {
        fn new() -> Self {
            Self {
                partition_active: Arc::new(AtomicBool::new(false)),
                packet_loss_rate: Arc::new(Mutex::new(0.0)),
            }
        }

        fn start_partition(&self, packet_loss_rate: f64) {
            *self.packet_loss_rate.lock().unwrap() = packet_loss_rate;
            self.partition_active.store(true, Ordering::Relaxed);
        }

        fn stop_partition(&self) {
            self.partition_active.store(false, Ordering::Relaxed);
            *self.packet_loss_rate.lock().unwrap() = 0.0;
        }

        fn is_partitioned(&self) -> bool {
            if !self.partition_active.load(Ordering::Relaxed) {
                return false;
            }

            let loss_rate = *self.packet_loss_rate.lock().unwrap();
            thread_rng().gen::<f64>() < loss_rate
        }
    }

    /// Test cache resilience under filesystem chaos
    #[test]
    fn test_filesystem_chaos_resilience() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let chaos_fs = ChaosFilesystem::new(
                temp_dir.path().to_path_buf(),
                0.1,  // 10% failure rate
                0.05, // 5% corruption rate
            );

            let config = UnifiedCacheConfig {
                max_entries: 100,
                max_memory_bytes: 1024 * 1024, // 1MB
                ..Default::default()
            };

            let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                .await
                .unwrap();

            let num_operations = 500;
            let mut successful_operations = 0;
            let mut failed_operations = 0;
            let mut data_corruption_detected = 0;

            println!(
                "Starting filesystem chaos test with {} operations",
                num_operations
            );

            for i in 0..num_operations {
                let key = format!("chaos_key_{}", i);
                let original_value = format!("chaos_value_{}_test_data", i).repeat(10);

                // Try to store value
                match cache.put(&key, original_value.as_bytes(), None).await {
                    Ok(_) => {
                        // Try to retrieve and verify
                        match cache.get::<Vec<u8>>(&key).await {
                            Ok(Some(retrieved)) => {
                                if retrieved == original_value.as_bytes() {
                                    successful_operations += 1;
                                } else {
                                    data_corruption_detected += 1;
                                }
                            }
                            Ok(None) => {
                                // Value not found - could be evicted or failed to store
                                failed_operations += 1;
                            }
                            Err(_) => {
                                failed_operations += 1;
                            }
                        }
                    }
                    Err(_) => {
                        failed_operations += 1;
                    }
                }

                // Small delay to allow for race conditions
                if i % 50 == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }

            let (failures, corruptions, latency_injections) = chaos_fs.get_stats();

            println!("Chaos test results:");
            println!("  Successful operations: {}", successful_operations);
            println!("  Failed operations: {}", failed_operations);
            println!("  Data corruption detected: {}", data_corruption_detected);
            println!("  Failures injected: {}", failures);
            println!("  Corruptions injected: {}", corruptions);
            println!("  Latency injections: {}", latency_injections);

            // System should handle chaos gracefully
            assert!(
                successful_operations > 0,
                "Some operations should succeed despite chaos"
            );
            assert!(failures > 0, "Chaos should have injected some failures");

            // Cache should remain functional
            let recovery_key = "recovery_test";
            let recovery_value = b"recovery_data";

            assert!(
                cache.put(recovery_key, recovery_value, None).await.is_ok(),
                "Cache should recover after chaos"
            );
        });
    }

    /// Test cache behavior under extreme memory pressure
    #[test]
    fn test_memory_pressure_chaos() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let memory_simulator = MemoryPressureSimulator::new(100); // 100MB pressure

            let config = UnifiedCacheConfig {
                max_memory_bytes: 50 * 1024 * 1024, // 50MB cache limit
                max_entries: 1000,
                ..Default::default()
            };

            let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                .await
                .unwrap();

            // Start memory pressure
            memory_simulator.start_pressure();

            let num_threads = 8;
            let operations_per_thread = 100;
            let barrier = Arc::new(Barrier::new(num_threads));
            let success_count = Arc::new(AtomicU32::new(0));
            let oom_count = Arc::new(AtomicU32::new(0));
            let cache = Arc::new(cache);

            let handles: Vec<_> = (0..num_threads)
                .map(|thread_id| {
                    let barrier = Arc::clone(&barrier);
                    let cache = Arc::clone(&cache);
                    let success_count = Arc::clone(&success_count);
                    let oom_count = Arc::clone(&oom_count);

                    tokio::spawn(async move {
                        barrier.wait();

                        for i in 0..operations_per_thread {
                            let key = format!("pressure_{}_{}", thread_id, i);
                            let value = vec![thread_id as u8; 10000]; // 10KB values

                            match cache.put(&key, &value, None).await {
                                Ok(_) => {
                                    // Try to read it back
                                    match cache.get::<Vec<u8>>(&key).await {
                                        Ok(Some(_)) => {
                                            success_count.fetch_add(1, Ordering::Relaxed);
                                        }
                                        Ok(None) => {
                                            // Evicted due to pressure
                                        }
                                        Err(_) => {
                                            oom_count.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                }
                                Err(CacheError::InsufficientMemory { .. }) => {
                                    oom_count.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    // Other errors under memory pressure
                                    oom_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    })
                })
                .collect();

            // Wait for completion with timeout
            for handle in handles {
                match timeout(Duration::from_secs(30), handle).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        println!("Task failed: {}", e);
                    }
                    Err(_) => {
                        println!("Task timed out under memory pressure");
                    }
                }
            }

            memory_simulator.stop_pressure();

            let total_success = success_count.load(Ordering::Relaxed);
            let total_oom = oom_count.load(Ordering::Relaxed);
            let total_operations = num_threads * operations_per_thread;

            println!("Memory pressure test results:");
            println!("  Total operations: {}", total_operations);
            println!("  Successful: {}", total_success);
            println!("  OOM/Pressure errors: {}", total_oom);

            // System should handle memory pressure gracefully
            assert!(
                total_success > 0 || total_oom > 0,
                "System should respond to memory pressure"
            );

            // Cache should still be functional after pressure
            let recovery_result = cache.put("post_pressure", b"test", None).await;
            assert!(
                recovery_result.is_ok(),
                "Cache should recover after memory pressure"
            );
        });
    }

    /// Test concurrent chaos with multiple failure modes
    #[test]
    fn test_multi_vector_chaos() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let chaos_fs = ChaosFilesystem::new(
                temp_dir.path().to_path_buf(),
                0.15, // 15% failure rate
                0.1,  // 10% corruption rate
            );
            let memory_pressure = MemoryPressureSimulator::new(50); // 50MB pressure
            let network_sim = NetworkPartitionSimulator::new();

            let cache = Arc::new(
                ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await
                    .unwrap(),
            );

            // Start all chaos conditions
            memory_pressure.start_pressure();
            network_sim.start_partition(0.3); // 30% packet loss

            let num_clients = 6;
            let duration = Duration::from_secs(10);
            let start_time = Instant::now();
            let barrier = Arc::new(Barrier::new(num_clients));

            let metrics = Arc::new(Mutex::new(HashMap::new()));

            let client_handles: Vec<_> = (0..num_clients)
                .map(|client_id| {
                    let cache = Arc::clone(&cache);
                    let barrier = Arc::clone(&barrier);
                    let metrics = Arc::clone(&metrics);
                    let network_sim = Arc::new(network_sim.clone());
                    let start_time = start_time.clone();

                    tokio::spawn(async move {
                        barrier.wait();

                        let mut client_metrics = HashMap::new();
                        client_metrics.insert("puts".to_string(), 0u64);
                        client_metrics.insert("gets".to_string(), 0u64);
                        client_metrics.insert("successes".to_string(), 0u64);
                        client_metrics.insert("failures".to_string(), 0u64);
                        client_metrics.insert("network_partitions".to_string(), 0u64);

                        let mut operation_id = 0;

                        while start_time.elapsed() < duration {
                            // Simulate network partition effects
                            if network_sim.is_partitioned() {
                                *client_metrics.get_mut("network_partitions").unwrap() += 1;
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                continue;
                            }

                            let key = format!("chaos_{}_{}", client_id, operation_id);
                            let value = format!("value_{}_{}", client_id, operation_id);

                            // Alternate between puts and gets
                            if operation_id % 2 == 0 {
                                *client_metrics.get_mut("puts").unwrap() += 1;
                                match cache.put(&key, value.as_bytes(), None).await {
                                    Ok(_) => {
                                        *client_metrics.get_mut("successes").unwrap() += 1;
                                    }
                                    Err(_) => {
                                        *client_metrics.get_mut("failures").unwrap() += 1;
                                    }
                                }
                            } else {
                                *client_metrics.get_mut("gets").unwrap() += 1;
                                match cache.get::<Vec<u8>>(&key).await {
                                    Ok(_) => {
                                        *client_metrics.get_mut("successes").unwrap() += 1;
                                    }
                                    Err(_) => {
                                        *client_metrics.get_mut("failures").unwrap() += 1;
                                    }
                                }
                            }

                            operation_id += 1;

                            // Small delay to avoid overwhelming the system
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }

                        if let Ok(mut global_metrics) = metrics.lock() {
                            global_metrics.insert(client_id, client_metrics);
                        }
                    })
                })
                .collect();

            // Wait for all clients
            for handle in client_handles {
                match timeout(duration + Duration::from_secs(5), handle).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        println!("Client failed: {}", e);
                    }
                    Err(_) => {
                        println!("Client timed out");
                    }
                }
            }

            // Stop chaos conditions
            memory_pressure.stop_pressure();
            network_sim.stop_partition();
            chaos_fs.disable();

            // Analyze results
            let final_metrics = metrics.lock().unwrap();
            let mut total_operations = 0u64;
            let mut total_successes = 0u64;
            let mut total_failures = 0u64;
            let mut total_partitions = 0u64;

            println!("Multi-vector chaos test results:");
            for (client_id, client_metrics) in final_metrics.iter() {
                let puts = client_metrics.get("puts").unwrap_or(&0);
                let gets = client_metrics.get("gets").unwrap_or(&0);
                let successes = client_metrics.get("successes").unwrap_or(&0);
                let failures = client_metrics.get("failures").unwrap_or(&0);
                let partitions = client_metrics.get("network_partitions").unwrap_or(&0);

                println!(
                    "  Client {}: puts={}, gets={}, successes={}, failures={}, partitions={}",
                    client_id, puts, gets, successes, failures, partitions
                );

                total_operations += puts + gets;
                total_successes += successes;
                total_failures += failures;
                total_partitions += partitions;
            }

            println!("Total summary:");
            println!("  Operations: {}", total_operations);
            println!("  Successes: {}", total_successes);
            println!("  Failures: {}", total_failures);
            println!("  Network partitions: {}", total_partitions);

            let (fs_failures, fs_corruptions, fs_latency) = chaos_fs.get_stats();
            println!("  FS failures: {}", fs_failures);
            println!("  FS corruptions: {}", fs_corruptions);
            println!("  FS latency injections: {}", fs_latency);

            // System should survive multi-vector chaos
            assert!(total_operations > 0, "Should have attempted operations");
            assert!(
                total_successes > 0 || total_failures > 0,
                "Should have some operation results"
            );
            assert!(
                total_partitions > 0 || fs_failures > 0,
                "Chaos should have been injected"
            );

            // Verify cache is still functional
            let final_test = cache.put("multi_chaos_recovery", b"recovered", None).await;
            assert!(
                final_test.is_ok(),
                "Cache should recover from multi-vector chaos"
            );
        });
    }

    /// Test graceful degradation under cascading failures
    #[test]
    fn test_cascading_failure_resilience() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let cache = Arc::new(
                ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await
                    .unwrap(),
            );

            // Stage 1: Normal operation
            println!("Stage 1: Normal operation");
            for i in 0..100 {
                let key = format!("normal_{}", i);
                let value = format!("value_{}", i);
                cache.put(&key, value.as_bytes(), None).await.unwrap();
            }

            let stats1 = cache.statistics().await.unwrap();
            println!("  Normal stats: {} operations", stats1.total_operations);

            // Stage 2: Introduce filesystem corruption
            println!("Stage 2: Filesystem corruption");
            let chaos_fs = ChaosFilesystem::new(
                temp_dir.path().to_path_buf(),
                0.5, // 50% failure rate
                0.3, // 30% corruption rate
            );

            let mut corruption_survived = 0;
            for i in 100..200 {
                let key = format!("corrupt_{}", i);
                let value = format!("value_{}", i);

                match cache.put(&key, value.as_bytes(), None).await {
                    Ok(_) => {
                        // Try to read back
                        if cache.get::<Vec<u8>>(&key).await.unwrap_or(None).is_some() {
                            corruption_survived += 1;
                        }
                    }
                    Err(_) => {
                        // Expected under corruption
                    }
                }
            }

            println!("  Operations survived corruption: {}", corruption_survived);

            // Stage 3: Add memory pressure
            println!("Stage 3: Memory pressure + corruption");
            let memory_pressure = MemoryPressureSimulator::new(100);
            memory_pressure.start_pressure();

            let mut pressure_survived = 0;
            for i in 200..300 {
                let key = format!("pressure_{}", i);
                let value = vec![i as u8; 1000]; // Larger values under pressure

                match cache.put(&key, &value, None).await {
                    Ok(_) => pressure_survived += 1,
                    Err(_) => {
                        // Expected under memory pressure
                    }
                }
            }

            println!("  Operations survived pressure: {}", pressure_survived);

            // Stage 4: Recovery phase
            println!("Stage 4: Recovery");
            chaos_fs.disable();
            memory_pressure.stop_pressure();

            // Allow some recovery time
            tokio::time::sleep(Duration::from_millis(500)).await;

            let mut recovery_operations = 0;
            for i in 300..350 {
                let key = format!("recovery_{}", i);
                let value = format!("recovered_{}", i);

                match cache.put(&key, value.as_bytes(), None).await {
                    Ok(_) => {
                        if cache.get::<Vec<u8>>(&key).await.unwrap_or(None).is_some() {
                            recovery_operations += 1;
                        }
                    }
                    Err(_) => {
                        // Should be rare in recovery
                    }
                }
            }

            println!("  Recovery operations: {}", recovery_operations);

            let final_stats = cache.statistics().await.unwrap();
            println!(
                "Final stats: {} total operations",
                final_stats.total_operations
            );

            // Verify graceful degradation
            assert!(
                corruption_survived < 90,
                "Should show degradation under corruption"
            );
            assert!(
                pressure_survived < corruption_survived,
                "Should degrade further under pressure"
            );
            assert!(
                recovery_operations > pressure_survived,
                "Should recover after chaos ends"
            );
            assert!(
                final_stats.total_operations > stats1.total_operations,
                "Stats should accumulate"
            );
        });
    }

    /// Test cache behavior during rapid configuration changes
    #[test]
    fn test_configuration_change_chaos() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();

            // Start with large cache
            let initial_config = UnifiedCacheConfig {
                max_memory_bytes: 10 * 1024 * 1024, // 10MB
                max_entries: 1000,
                ttl_secs: Some(Duration::from_secs(60)),
                ..Default::default()
            };

            let cache = ProductionCache::new(temp_dir.path().to_path_buf(), initial_config)
                .await
                .unwrap();

            // Fill cache
            for i in 0..500 {
                let key = format!("config_test_{}", i);
                let value = vec![i as u8; 1000];
                let _ = cache.put(&key, &value, None).await;
            }

            let initial_stats = cache.statistics().await.unwrap();
            println!("Initial cache entries: {}", initial_stats.entries);

            // Simulate configuration changes by creating new cache instances
            // with different configurations on the same directory
            let configs = vec![
                UnifiedCacheConfig {
                    max_memory_bytes: 1024 * 1024, // 1MB - drastic reduction
                    max_entries: 100,
                    ttl_secs: Some(Duration::from_secs(5)), // Short TTL
                    ..Default::default()
                },
                UnifiedCacheConfig {
                    max_memory_bytes: 50 * 1024 * 1024, // 50MB - increase
                    max_entries: 5000,
                    ttl_secs: None, // No TTL
                    compression_enabled: true,
                    ..Default::default()
                },
                UnifiedCacheConfig {
                    max_memory_bytes: 512 * 1024, // 512KB - very small
                    max_entries: 10,
                    ttl_secs: Some(Duration::from_millis(100)), // Very short TTL
                    ..Default::default()
                },
            ];

            for (i, config) in configs.iter().enumerate() {
                println!(
                    "Configuration change {}: max_memory={} bytes, max_entries={}",
                    i + 1,
                    config.max_memory_bytes,
                    config.max_entries
                );

                // Create new cache with different config
                let new_cache =
                    match ProductionCache::new(temp_dir.path().to_path_buf(), config.clone()).await
                    {
                        Ok(cache) => cache,
                        Err(e) => {
                            println!("Failed to create cache with config {}: {}", i + 1, e);
                            continue;
                        }
                    };

                // Test basic operations
                let mut operations_succeeded = 0;
                for j in 0..50 {
                    let key = format!("config_change_{}_{}", i, j);
                    let value = vec![j as u8; 100];

                    if new_cache.put(&key, &value, None).await.is_ok() {
                        if new_cache
                            .get::<Vec<u8>>(&key)
                            .await
                            .unwrap_or(None)
                            .is_some()
                        {
                            operations_succeeded += 1;
                        }
                    }
                }

                let stats = new_cache.statistics().await.unwrap();
                println!(
                    "  Operations succeeded: {}, Current entries: {}",
                    operations_succeeded, stats.entries
                );

                // Wait a bit for TTL effects
                if config.ttl_secs.is_some() {
                    tokio::time::sleep(config.ttl_secs.unwrap() + Duration::from_millis(100)).await;

                    let post_ttl_stats = new_cache.statistics().await.unwrap();
                    println!("  Post-TTL entries: {}", post_ttl_stats.entries);
                }
            }

            // Verify final cache is still functional
            let final_cache =
                ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                    .await
                    .unwrap();

            let final_test = final_cache.put("final_test", b"final_value", None).await;
            assert!(
                final_test.is_ok(),
                "Cache should remain functional after config chaos"
            );

            let final_stats = final_cache.statistics().await.unwrap();
            println!("Final cache state: {} entries", final_stats.entries);
        });
    }
}
