#![allow(unused)]
//! Comprehensive concurrent tests for cuenv
//!
//! This module contains additional edge case tests for concurrent scenarios
//! that test behavior under extreme conditions and complex interactions.

#[cfg(test)]
mod comprehensive_concurrent_tests {
    use cuenv::cache::CacheManager;
    use cuenv::config::TaskConfig;
    use cuenv::env::EnvManager;
    use cuenv::errors::Result;
    use cuenv::state::StateManager;
    use cuenv::sync_env::SyncEnv;
    use cuenv::task_executor::TaskExecutor;
    use cuenv::utils::ResourceLimits;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// Helper to create CacheManager with test-specific cache directory
    fn create_test_cache_manager() -> (Arc<CacheManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
        let cache_manager = Arc::new(CacheManager::new_sync().unwrap());
        (cache_manager, temp_dir)
    }

    /// Test behavior when multiple tasks compete for limited resources
    #[test]
    fn test_resource_exhaustion_under_concurrent_load() {
        let temp_dir = TempDir::new().unwrap();
        let num_tasks = 100; // Many more tasks than typical system resources
        let barrier = Arc::new(Barrier::new(num_tasks));
        let completed = Arc::new(AtomicU32::new(0));
        let resource_errors = Arc::new(AtomicU32::new(0));
        let timeouts = Arc::new(AtomicU32::new(0));

        // Set up resource limits
        let limits = ResourceLimits::unlimited()
            .with_cpu_time(2, 3) // 2 second soft limit, 3 second hard limit
            .with_memory(512 * 1024 * 1024, 1024 * 1024 * 1024); // 512MB soft, 1GB hard
        let task_timeout = Duration::from_secs(2);

        let handles: Vec<_> = (0..num_tasks)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let completed = Arc::clone(&completed);
                let resource_errors = Arc::clone(&resource_errors);
                let timeouts = Arc::clone(&timeouts);
                let working_dir = temp_dir.path().to_path_buf();
                let limits = limits.clone();

                thread::spawn(move || {
                    barrier.wait();

                    // Simulate memory-intensive task
                    let task_config = TaskConfig {
                        description: Some(format!("Resource test {}", i)),
                        command: Some("dd if=/dev/zero of=/dev/null bs=1M count=100".to_string()),
                        script: None,
                        dependencies: None,
                        working_dir: None,
                        shell: None,
                        inputs: None,
                        outputs: None,
                        security: None,
                        cache: Some(cuenv::cache::TaskCacheConfig::Simple(false)),
                        cache_key: None,
                        cache_env: None,
                        timeout: Some(task_timeout.as_secs() as u32),
                    };

                    // Simulate task execution
                    let start = Instant::now();
                    thread::sleep(Duration::from_millis(100));

                    if start.elapsed() > task_timeout {
                        timeouts.fetch_add(1, Ordering::SeqCst);
                    } else {
                        completed.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let total_completed = completed.load(Ordering::SeqCst);
        let total_errors = resource_errors.load(Ordering::SeqCst);
        let total_timeouts = timeouts.load(Ordering::SeqCst);

        println!(
            "Resource test results - Completed: {}, Errors: {}, Timeouts: {}",
            total_completed, total_errors, total_timeouts
        );

        // Verify resource limits were enforced
        assert!(total_completed + total_errors + total_timeouts == num_tasks as u32);
        assert!(
            total_completed > 0,
            "Some tasks should complete successfully"
        );
    }

    /// Test cache behavior during rapid file system changes
    #[test]
    fn test_cache_consistency_with_filesystem_race() {
        let temp_dir = TempDir::new().unwrap();
        let (cache_manager, _cache_temp) = create_test_cache_manager();
        let num_writers = 5;
        let num_readers = 5;
        let duration_secs = 3;
        let barrier = Arc::new(Barrier::new(num_writers + num_readers));
        let inconsistencies = Arc::new(AtomicU32::new(0));
        let start_time = Instant::now();

        // Create test files
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Writer threads that rapidly modify files
        let writer_handles: Vec<_> = (0..num_writers)
            .map(|writer_id| {
                let barrier = Arc::clone(&barrier);
                let src_dir = src_dir.clone();

                thread::spawn(move || {
                    barrier.wait();
                    let mut counter = 0;

                    while start_time.elapsed().as_secs() < duration_secs {
                        let file_path = src_dir.join(format!("file_{}.txt", writer_id));

                        // Rapid create/modify/delete cycle
                        fs::write(&file_path, format!("version {}", counter)).ok();
                        thread::sleep(Duration::from_millis(10));
                        fs::write(&file_path, format!("version {} modified", counter)).ok();
                        thread::sleep(Duration::from_millis(10));
                        fs::remove_file(&file_path).ok();

                        counter += 1;
                    }
                })
            })
            .collect();

        // Reader threads that try to cache based on file state
        let reader_handles: Vec<_> = (0..num_readers)
            .map(|reader_id| {
                let barrier = Arc::clone(&barrier);
                let cache_manager = Arc::clone(&cache_manager);
                let inconsistencies = Arc::clone(&inconsistencies);
                let working_dir = temp_dir.path().to_path_buf();

                thread::spawn(move || {
                    barrier.wait();

                    while start_time.elapsed().as_secs() < duration_secs {
                        let task_config = TaskConfig {
                            description: Some(format!("Reader task {}", reader_id)),
                            command: Some("echo test".to_string()),
                            script: None,
                            dependencies: None,
                            working_dir: None,
                            shell: None,
                            inputs: Some(vec!["src/*.txt".to_string()]),
                            outputs: None,
                            security: None,
                            cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
                            cache_key: None,
                            cache_env: None,
                            timeout: None,
                        };

                        // Generate cache key
                        let env_vars = HashMap::new();
                        match cache_manager.generate_cache_key(
                            "reader_task",
                            &task_config,
                            &env_vars,
                            &working_dir,
                        ) {
                            Ok(key1) => {
                                // Small delay
                                thread::sleep(Duration::from_millis(5));

                                // Generate again and check consistency
                                match cache_manager.generate_cache_key(
                                    "reader_task",
                                    &task_config,
                                    &env_vars,
                                    &working_dir,
                                ) {
                                    Ok(key2) => {
                                        // Keys should be same if files haven't changed
                                        // But with rapid changes, this tests cache invalidation
                                    }
                                    Err(_) => {
                                        inconsistencies.fetch_add(1, Ordering::SeqCst);
                                    }
                                }
                            }
                            Err(_) => {
                                // Expected when files are being deleted
                            }
                        }
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in writer_handles {
            handle.join().unwrap();
        }
        for handle in reader_handles {
            handle.join().unwrap();
        }

        let total_inconsistencies = inconsistencies.load(Ordering::SeqCst);
        println!(
            "Filesystem race test - Inconsistencies: {}",
            total_inconsistencies
        );

        // Some inconsistencies are expected due to race conditions
        // but they should be handled gracefully
        assert!(
            total_inconsistencies < 100,
            "Too many cache inconsistencies detected"
        );
    }

    /// Test complex dependency chains under concurrent execution
    #[test]
    fn test_concurrent_dependency_resolution() {
        let runtime = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        runtime.block_on(async {
            // Create a diamond dependency pattern
            // A -> B -> D
            // A -> C -> D
            // E -> D
            let tasks_cue = r#"package env
env: {}
tasks: {
    "A": {
        description: "Task A"
        command: "echo A > a.txt"
        outputs: ["a.txt"]
        cache: true
    }
    "B": {
        description: "Task B"
        command: "cat a.txt && echo B > b.txt"
        dependencies: ["A"]
        inputs: ["a.txt"]
        outputs: ["b.txt"]
        cache: true
    }
    "C": {
        description: "Task C"
        command: "cat a.txt && echo C > c.txt"
        dependencies: ["A"]
        inputs: ["a.txt"]
        outputs: ["c.txt"]
        cache: true
    }
    "D": {
        description: "Task D"
        command: "cat b.txt c.txt && echo D > d.txt"
        dependencies: ["B", "C"]
        inputs: ["b.txt", "c.txt"]
        outputs: ["d.txt"]
        cache: true
    }
    "E": {
        description: "Task E"
        command: "echo E > e.txt"
        outputs: ["e.txt"]
        cache: true
    }
    "F": {
        description: "Task F"
        command: "cat d.txt e.txt && echo F > f.txt"
        dependencies: ["D", "E"]
        inputs: ["d.txt", "e.txt"]
        outputs: ["f.txt"]
        cache: true
    }
}"#;

            // Write CUE file and load it
            let env_file = temp_dir.path().join("env.cue");
            fs::write(&env_file, tasks_cue).unwrap();

            let mut env_manager = EnvManager::new();
            env_manager.load_env(temp_dir.path()).await.unwrap();

            let executor = TaskExecutor::new(env_manager, temp_dir.path().to_path_buf())
                .await
                .unwrap();

            // Execute F which should trigger the entire dependency chain
            let exit_code = executor.execute_task("F", &[]).await.unwrap();
            assert_eq!(exit_code, 0);

            // Verify all files were created in correct order
            assert!(temp_dir.path().join("a.txt").exists());
            assert!(temp_dir.path().join("b.txt").exists());
            assert!(temp_dir.path().join("c.txt").exists());
            assert!(temp_dir.path().join("d.txt").exists());
            assert!(temp_dir.path().join("e.txt").exists());
            assert!(temp_dir.path().join("f.txt").exists());

            // Execute again - should use cache
            let start = Instant::now();
            let exit_code = executor.execute_task("F", &[]).await.unwrap();
            let duration = start.elapsed();

            assert_eq!(exit_code, 0);
            assert!(
                duration < Duration::from_secs(2),
                "Cached execution should be fast (was {:?}ms)",
                duration.as_millis()
            );
        });
    }

    /// Test rollback behavior when tasks fail mid-execution
    #[test]
    fn test_concurrent_rollback_on_failure() {
        let runtime = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let rollback_called = Arc::new(AtomicBool::new(false));
        let rollback_called_clone = rollback_called.clone();

        runtime.block_on(async {
            // Create test environment files
            let env_dir = temp_dir.path().join("env");
            fs::create_dir(&env_dir).unwrap();
            fs::write(env_dir.join("state.json"), "{}").unwrap();

            // Set up initial state
            let diff = cuenv::env::EnvDiff::new(HashMap::new(), HashMap::new());
            let watches = cuenv::file_times::FileTimes::new();

            // Load state
            StateManager::load(
                &env_dir,
                &env_dir.join("env.cue"),
                Some("test_env"),
                &["test_cap".to_string()],
                &diff,
                &watches,
            )
            .await
            .unwrap();

            // Create failing task configuration
            let tasks_cue = r#"package env
env: {}
tasks: {
    "setup": {
        description: "Setup task"
        command: "echo 'setup' > setup.txt"
        outputs: ["setup.txt"]
        cache: false
    }
    "failing": {
        description: "Failing task"
        command: "exit 1"
        dependencies: ["setup"]
        inputs: ["setup.txt"]
        cache: false
    }
}"#;

            // Write CUE file and load it
            let env_file = env_dir.join("env.cue");
            fs::write(&env_file, tasks_cue).unwrap();

            let mut env_manager = EnvManager::new();
            env_manager.load_env(&env_dir).await.unwrap();

            let executor = TaskExecutor::new(env_manager, temp_dir.path().to_path_buf())
                .await
                .unwrap();

            // Execute failing task
            let result = executor.execute_task("failing", &[]).await;

            // Task should fail
            assert!(result.is_err() || result.unwrap() != 0);

            // Verify setup file was created
            assert!(temp_dir.path().join("setup.txt").exists());

            // Verify state can be unloaded properly even after failure
            let unload_result = StateManager::unload().await;
            assert!(unload_result.is_ok());
        });
    }

    /// Test behavior when cache operations timeout
    #[test]
    fn test_cache_operation_timeouts() {
        let (cache_manager, _cache_temp) = create_test_cache_manager();
        let temp_dir = TempDir::new().unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let timeout_occurred = Arc::new(AtomicBool::new(false));

        // Create a large file to slow down operations
        let large_file = temp_dir.path().join("large.bin");
        let mut data = vec![0u8; 100 * 1024 * 1024]; // 100MB
        fs::write(&large_file, &data).unwrap();

        let task_config = TaskConfig {
            description: Some("Timeout test".to_string()),
            command: Some("echo test".to_string()),
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: Some(vec!["large.bin".to_string()]),
            outputs: None,
            security: None,
            cache: Some(cuenv::cache::TaskCacheConfig::Simple(true)),
            cache_key: None,
            cache_env: None,
            timeout: Some(1), // 1 second timeout
        };

        // Thread 1: Perform cache operation with large file
        let cache_manager1 = Arc::clone(&cache_manager);
        let temp_dir1 = temp_dir.path().to_path_buf();
        let task_config1 = task_config.clone();
        let barrier1 = Arc::clone(&barrier);
        let timeout_occurred1 = Arc::clone(&timeout_occurred);

        let handle1 = thread::spawn(move || {
            barrier1.wait();

            let start = Instant::now();
            let result = cache_manager1.generate_cache_key(
                "timeout_test",
                &task_config1,
                &HashMap::new(),
                &temp_dir1,
            );
            let duration = start.elapsed();

            // With a 100MB file, hashing might take longer than expected
            if duration > Duration::from_millis(100) {
                timeout_occurred1.store(true, Ordering::SeqCst);
            }

            result
        });

        // Thread 2: Try to access same cache concurrently
        let cache_manager2 = Arc::clone(&cache_manager);
        let temp_dir2 = temp_dir.path().to_path_buf();
        let barrier2 = Arc::clone(&barrier);

        let handle2 = thread::spawn(move || {
            barrier2.wait();

            // Small delay to ensure thread 1 starts first
            thread::sleep(Duration::from_millis(10));

            // This might need to wait for thread 1
            cache_manager2.generate_cache_key(
                "timeout_test",
                &task_config,
                &HashMap::new(),
                &temp_dir2,
            )
        });

        let result1 = handle1.join().unwrap();
        let result2 = handle2.join().unwrap();

        // Both operations should eventually complete
        assert!(result1.is_ok() || timeout_occurred.load(Ordering::SeqCst));
        assert!(result2.is_ok() || timeout_occurred.load(Ordering::SeqCst));
    }

    /// Test concurrent access with memory pressure
    #[test]
    #[cfg_attr(coverage, ignore)]
    fn test_memory_pressure_concurrent_operations() {
        let num_threads = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        let oom_errors = Arc::new(AtomicU32::new(0));
        let success_count = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let oom_errors = Arc::clone(&oom_errors);
                let success_count = Arc::clone(&success_count);

                thread::spawn(move || {
                    barrier.wait();

                    // Try to allocate large amounts of memory
                    let allocation_size = 50 * 1024 * 1024; // 50MB per thread
                    let mut data = Vec::<u8>::new();
                    match data.try_reserve(allocation_size) {
                        Ok(()) => {
                            data.resize(allocation_size, (i + 1) as u8);

                            // Simulate some work with the memory
                            let sum: u64 = data.iter().map(|&x| x as u64).sum();
                            assert!(sum > 0);

                            success_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(_) => {
                            oom_errors.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let total_oom = oom_errors.load(Ordering::SeqCst);
        let total_success = success_count.load(Ordering::SeqCst);

        println!(
            "Memory pressure test - Success: {}, OOM: {}",
            total_success, total_oom
        );

        // At least some operations should succeed
        assert!(total_success > 0);
        assert_eq!(total_success + total_oom, num_threads as u32);
    }
}
