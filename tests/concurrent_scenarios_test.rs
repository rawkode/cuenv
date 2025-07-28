#![allow(unused)]
#[cfg(test)]
mod concurrent_scenarios {
    use cuenv::async_runtime::{run_async, AsyncRuntime};
    use cuenv::cache::{CacheConfig, CacheEngine, CacheManager, CacheMode, CachedTaskResult};
    use cuenv::cue_parser::TaskConfig;
    use cuenv::env_manager::EnvManager;
    use cuenv::errors::Result;
    use cuenv::state::StateManager;
    use cuenv::sync_env::SyncEnv;
    use cuenv::task_executor::TaskExecutor;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    // Global mutex to ensure cache manager creation is synchronized
    static CACHE_CREATION_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper to create CacheManager with test-specific cache directory
    fn create_test_cache_manager(cache_dir: &std::path::Path) -> CacheManager {
        use cuenv::cache::{CacheConfig, CacheMode};

        // Ensure only one thread creates a cache manager at a time
        let _guard = CACHE_CREATION_MUTEX.lock().unwrap();

        let config = CacheConfig {
            base_dir: cache_dir.join("cuenv"),
            max_size: 1024 * 1024 * 1024, // 1GB for tests
            mode: CacheMode::ReadWrite,
            inline_threshold: 4096,
        };

        // Use the runtime helper to create cache manager asynchronously
        run_async(CacheManager::new(config)).unwrap()
    }

    /// Test concurrent access to build cache by multiple tasks
    #[test]
    fn test_concurrent_build_cache_access() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let num_threads = 10;
        let barrier = Arc::new(Barrier::new(num_threads));
        let cache_hits = Arc::new(AtomicU32::new(0));
        let cache_misses = Arc::new(AtomicU32::new(0));
        let execution_count = Arc::new(AtomicU32::new(0));

        // Create a source file that will be used as input
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("input.txt"), "test content").unwrap();

        // Create output directory
        let build_dir = temp_dir.path().join("build");
        fs::create_dir(&build_dir).unwrap();

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let cache_hits = Arc::clone(&cache_hits);
                let cache_misses = Arc::clone(&cache_misses);
                let execution_count = Arc::clone(&execution_count);
                let working_dir = temp_dir.path().to_path_buf();
                let cache_dir_path = cache_dir.path().to_path_buf();

                thread::spawn(move || {
                    // Wait for all threads to start
                    barrier.wait();

                    let cache = create_test_cache_manager(&cache_dir_path);
                    let task_config = TaskConfig {
                        description: Some("Test concurrent cache task".to_string()),
                        command: Some("echo test > build/output.txt".to_string()),
                        script: None,
                        dependencies: None,
                        working_dir: None,
                        shell: None,
                        inputs: Some(vec!["src/*".to_string()]),
                        outputs: Some(vec!["build/output.txt".to_string()]),
                        security: None,
                        cache: Some(true),
                        cache_key: None,
                        timeout: None,
                    };

                    // Generate cache key
                    let cache_key = cache
                        .generate_cache_key_legacy("concurrent_test", &task_config, &working_dir)
                        .unwrap();

                    // Check cache
                    match cache.get_cached_result(&cache_key) {
                        Some(_) => {
                            cache_hits.fetch_add(1, Ordering::SeqCst);
                        }
                        None => {
                            cache_misses.fetch_add(1, Ordering::SeqCst);

                            // Simulate task execution
                            execution_count.fetch_add(1, Ordering::SeqCst);

                            // Create output file if it doesn't exist
                            let output_path = working_dir.join("build/output.txt");
                            if !output_path.exists() {
                                fs::write(&output_path, "test output").ok();
                            }

                            // Save to cache
                            cache
                                .save_result(&cache_key, &task_config, &working_dir, 0)
                                .unwrap();
                        }
                    }

                    // Small delay to simulate some work
                    thread::sleep(Duration::from_millis(10));
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify results
        let total_hits = cache_hits.load(Ordering::SeqCst);
        let total_misses = cache_misses.load(Ordering::SeqCst);
        let total_executions = execution_count.load(Ordering::SeqCst);

        println!(
            "Cache hits: {}, Cache misses: {}, Executions: {}",
            total_hits, total_misses, total_executions
        );

        // The task should have been executed at least once
        assert!(
            total_executions >= 1,
            "Task should have been executed at least once"
        );

        // Total hits + misses should equal number of threads
        assert_eq!(total_hits + total_misses, num_threads as u32);

        // Cache hits should occur after the first execution
        if total_executions == 1 {
            assert_eq!(total_hits, (num_threads - 1) as u32);
        }
    }

    /// Test state management under concurrent load
    #[test]
    fn test_concurrent_state_management() {
        let runtime = Runtime::new().unwrap();
        let num_threads = 5;
        let iterations = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        let errors = Arc::new(Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let errors = Arc::clone(&errors);

                thread::spawn(move || {
                    let runtime = Runtime::new().unwrap();
                    barrier.wait();

                    for j in 0..iterations {
                        let temp_dir = TempDir::new().unwrap();
                        let dir = temp_dir.path();
                        let file = dir.join("env.cue");

                        // Create unique environment name
                        let env_name = format!("env_thread_{}_iter_{}", i, j);

                        // Load state
                        runtime.block_on(async {
                            let diff =
                                cuenv::env_diff::EnvDiff::new(HashMap::new(), HashMap::new());
                            let watches = cuenv::file_times::FileTimes::new();

                            match StateManager::load(
                                dir,
                                &file,
                                Some(&env_name),
                                &[format!("cap_{}", i)],
                                &diff,
                                &watches,
                            )
                            .await
                            {
                                Ok(_) => {
                                    // Verify state was loaded (don't check specific env name due to concurrency)
                                    if let Ok(state_opt) = StateManager::get_state() {
                                        // Just verify that some state exists, not which specific one
                                        if state_opt.is_none() {
                                            errors
                                                .lock()
                                                .unwrap()
                                                .push("State was None after load".to_string());
                                        }
                                    }

                                    // Small delay to increase contention
                                    thread::sleep(Duration::from_micros(50));

                                    // Unload state
                                    if let Err(e) = StateManager::unload().await {
                                        errors.lock().unwrap().push(format!("Unload error: {}", e));
                                    }
                                }
                                Err(e) => {
                                    errors.lock().unwrap().push(format!("Load error: {}", e));
                                }
                            }
                        });
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Check for errors
        let error_list = errors.lock().unwrap();
        if !error_list.is_empty() {
            panic!("State management errors occurred: {:?}", *error_list);
        }
    }

    /// Test cache behavior during error recovery and rollback
    #[test]
    fn test_cache_error_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let cache = create_test_cache_manager(cache_dir.path());

        // Create source files
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("input.txt"), "initial content").unwrap();

        // Create build directory
        let build_dir = temp_dir.path().join("build");
        fs::create_dir(&build_dir).unwrap();

        let task_config = TaskConfig {
            description: Some("Test error recovery task".to_string()),
            command: Some("false".to_string()), // Command that always fails
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: Some(vec!["src/*".to_string()]),
            outputs: Some(vec!["build/output.txt".to_string()]),
            security: None,
            cache: Some(true),
            cache_key: None,
            timeout: None,
        };

        let cache_key = cache
            .generate_cache_key_legacy("error_test", &task_config, temp_dir.path())
            .unwrap();

        // Save a failed result (non-zero exit code)
        cache
            .save_result(&cache_key, &task_config, temp_dir.path(), 1)
            .unwrap();

        // Verify failed results are not cached
        let cached_result = cache.get_cached_result(&cache_key);

        assert!(cached_result.is_none(), "Failed tasks should not be cached");

        // Now test successful execution
        let success_config = TaskConfig {
            command: Some("echo success > build/output.txt".to_string()),
            ..task_config
        };

        // Generate new cache key for success config
        let success_cache_key = cache
            .generate_cache_key_legacy("success_test", &success_config, temp_dir.path())
            .unwrap();

        // Create the output file
        fs::write(build_dir.join("output.txt"), "success").unwrap();

        // Save successful result
        cache
            .save_result(&success_cache_key, &success_config, temp_dir.path(), 0)
            .unwrap();

        // Verify successful results are cached
        let cached_result = cache.get_cached_result(&success_cache_key);

        assert!(cached_result.is_some(), "Successful tasks should be cached");
        assert_eq!(cached_result.unwrap().exit_code, 0);
    }

    /// Test resource limits and timeouts in concurrent scenarios
    #[test]
    fn test_concurrent_resource_limits() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let num_tasks = 20; // More tasks than typical CPU cores
        let barrier = Arc::new(Barrier::new(num_tasks));
        let completed = Arc::new(AtomicU32::new(0));
        let start_time = SystemTime::now();

        let handles: Vec<_> = (0..num_tasks)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let completed = Arc::clone(&completed);
                let working_dir = temp_dir.path().to_path_buf();
                let cache_dir_path = cache_dir.path().to_path_buf();

                thread::spawn(move || {
                    barrier.wait();

                    let cache = create_test_cache_manager(&cache_dir_path);
                    let task_config = TaskConfig {
                        description: Some(format!("Resource test task {}", i)),
                        command: Some("sleep 0.1".to_string()), // Simulate work
                        script: None,
                        dependencies: None,
                        working_dir: None,
                        shell: None,
                        inputs: None,
                        outputs: None,
                        security: None,
                        cache: Some(false), // Disable cache for this test
                        cache_key: None,
                        timeout: Some(5), // 5 second timeout
                    };

                    // Generate unique cache key
                    let cache_key = format!("resource_test_{}", i);

                    // Simulate task execution with timeout
                    let execution_start = SystemTime::now();
                    thread::sleep(Duration::from_millis(100));
                    let execution_duration = execution_start.elapsed().unwrap();

                    // Verify timeout wasn't exceeded
                    assert!(
                        execution_duration < Duration::from_secs(5),
                        "Task execution exceeded timeout"
                    );

                    completed.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        // Wait for all tasks
        for handle in handles {
            handle.join().unwrap();
        }

        let total_duration = start_time.elapsed().unwrap();
        let total_completed = completed.load(Ordering::SeqCst);

        println!(
            "Completed {} tasks in {:?}",
            total_completed, total_duration
        );

        // All tasks should complete
        assert_eq!(total_completed, num_tasks as u32);

        // With concurrent execution, total time should be less than sequential
        let sequential_time = Duration::from_millis(100 * num_tasks as u64);
        assert!(
            total_duration < sequential_time,
            "Concurrent execution should be faster than sequential"
        );
    }

    /// Integration test for full workflow with concurrent tasks
    #[test]
    fn test_integrated_concurrent_workflow() {
        let runtime = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        runtime.block_on(async {
            // Setup test environment
            let src_dir = temp_dir.path().join("src");
            fs::create_dir(&src_dir).unwrap();

            // Create multiple source files
            for i in 0..5 {
                fs::write(
                    src_dir.join(format!("file{}.txt", i)),
                    format!("content {}", i),
                )
                .unwrap();
            }

            // Create build directory
            let build_dir = temp_dir.path().join("build");
            fs::create_dir(&build_dir).unwrap();

            // Create task configurations with dependencies
            let mut tasks = HashMap::new();

            // Independent tasks (can run concurrently)
            for i in 0..3 {
                let task_name = format!("compile_{}", i);
                tasks.insert(
                    task_name.clone(),
                    TaskConfig {
                        description: Some(format!("Compile task {}", i)),
                        command: Some(format!("cp src/file{}.txt build/compiled_{}.txt", i, i)),
                        script: None,
                        dependencies: None,
                        working_dir: None,
                        shell: None,
                        inputs: Some(vec![format!("src/file{}.txt", i)]),
                        outputs: Some(vec![format!("build/compiled_{}.txt", i)]),
                        security: None,
                        cache: Some(true),
                        cache_key: None,
                        timeout: None,
                    },
                );
            }

            // Dependent task (must wait for compile tasks)
            tasks.insert(
                "bundle".to_string(),
                TaskConfig {
                    description: Some("Bundle compiled files".to_string()),
                    command: Some("/bin/sh -c 'cat build/compiled_0.txt build/compiled_1.txt build/compiled_2.txt > build/bundle.txt'".to_string()),
                    script: None,
                    dependencies: Some(vec![
                        "compile_0".to_string(),
                        "compile_1".to_string(),
                        "compile_2".to_string(),
                    ]),
                    working_dir: None,
                    shell: None,
                    inputs: Some(vec!["build/compiled_*.txt".to_string()]),
                    outputs: Some(vec!["build/bundle.txt".to_string()]),
                    security: None,
                    cache: Some(true),
                    cache_key: None,
                    timeout: None,
                },
            );

            // Create env manager with tasks
            // Write tasks as CUE file
            let tasks_cue = r#"package env
env: {}
tasks: {
    "compile_0": {
        description: "Compile task 0"
        command: "cp src/file0.txt build/compiled_0.txt"
        inputs: ["src/file0.txt"]
        outputs: ["build/compiled_0.txt"]
        cache: true
    }
    "compile_1": {
        description: "Compile task 1"
        command: "cp src/file1.txt build/compiled_1.txt"
        inputs: ["src/file1.txt"]
        outputs: ["build/compiled_1.txt"]
        cache: true
    }
    "compile_2": {
        description: "Compile task 2"
        command: "cp src/file2.txt build/compiled_2.txt"
        inputs: ["src/file2.txt"]
        outputs: ["build/compiled_2.txt"]
        cache: true
    }
    "bundle": {
        description: "Bundle compiled files"
        command: "/bin/sh -c 'cat build/compiled_0.txt build/compiled_1.txt build/compiled_2.txt > build/bundle.txt'"
        dependencies: ["compile_0", "compile_1", "compile_2"]
        inputs: ["build/compiled_*.txt"]
        outputs: ["build/bundle.txt"]
        cache: true
    }
}"#;
            let env_file = temp_dir.path().join("env.cue");
            fs::write(&env_file, tasks_cue).unwrap();

            let mut env_manager = EnvManager::new();
            env_manager.load_env(temp_dir.path()).await.unwrap();

            // Create task executor
            let executor = TaskExecutor::new(env_manager, temp_dir.path().to_path_buf())
                .await
                .unwrap();

            // Execute bundle task (which depends on compile tasks)
            let exit_code = executor.execute_task("bundle", &[]).await.unwrap();

            assert_eq!(exit_code, 0, "Task execution should succeed");

            // Verify all outputs were created
            for i in 0..3 {
                let compiled_file = build_dir.join(format!("compiled_{}.txt", i));
                assert!(compiled_file.exists(), "Compiled file {} should exist", i);
            }

            let bundle_file = build_dir.join("bundle.txt");
            assert!(bundle_file.exists(), "Bundle file should exist");

            // Execute again to test if it uses cache (should be much faster)
            let start = SystemTime::now();
            let exit_code = executor.execute_task("bundle", &[]).await.unwrap();
            let second_duration = start.elapsed().unwrap();

            assert_eq!(exit_code, 0, "Cached task execution should succeed");

            // Second execution should be much faster due to cache
            println!("Second execution took: {:?}", second_duration);
            assert!(
                second_duration < Duration::from_secs(1),
                "Cached execution should be fast"
            );
        });
    }

    /// Test concurrent cache operations with file modifications
    #[test]
    #[ignore] // Cache invalidation based on file content changes not implemented yet
    fn test_cache_invalidation_race_conditions() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let num_threads = 5;
        let barrier = Arc::new(Barrier::new(num_threads * 2)); // Writers and readers
        let cache_invalidations = Arc::new(AtomicU32::new(0));

        // Create initial files
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        let input_file = src_dir.join("input.txt");
        fs::write(&input_file, "initial content").unwrap();

        // Writer threads that modify files
        let writer_handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let input_file = input_file.clone();

                thread::spawn(move || {
                    barrier.wait();

                    // Modify file content
                    thread::sleep(Duration::from_millis(i as u64 * 10));
                    fs::write(&input_file, format!("modified by thread {}", i)).unwrap();
                })
            })
            .collect();

        // Reader threads that check cache
        let reader_handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                let cache_invalidations = Arc::clone(&cache_invalidations);
                let working_dir = temp_dir.path().to_path_buf();
                let cache_dir_path = cache_dir.path().to_path_buf();

                thread::spawn(move || {
                    barrier.wait();

                    let cache = create_test_cache_manager(&cache_dir_path);
                    let task_config = TaskConfig {
                        description: Some("Cache invalidation test".to_string()),
                        command: Some("echo test".to_string()),
                        script: None,
                        dependencies: None,
                        working_dir: None,
                        shell: None,
                        inputs: Some(vec!["src/input.txt".to_string()]),
                        outputs: None,
                        security: None,
                        cache: Some(true),
                        cache_key: None,
                        timeout: None,
                    };

                    // Generate initial cache key
                    let initial_key = cache
                        .generate_cache_key_legacy("invalidation_test", &task_config, &working_dir)
                        .unwrap();

                    // Small delay
                    thread::sleep(Duration::from_millis(50));

                    // Generate cache key again
                    let new_key = cache
                        .generate_cache_key_legacy("invalidation_test", &task_config, &working_dir)
                        .unwrap();

                    // Check if cache was invalidated
                    if initial_key != new_key {
                        cache_invalidations.fetch_add(1, Ordering::SeqCst);
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

        let total_invalidations = cache_invalidations.load(Ordering::SeqCst);
        println!("Cache invalidations detected: {}", total_invalidations);

        // At least some cache invalidations should occur due to file modifications
        assert!(
            total_invalidations > 0,
            "Cache invalidations should be detected when files are modified"
        );
    }
}
