#![allow(unused)]
#[cfg(test)]
mod state_race_condition_tests {
    use cuenv::env::EnvDiff;
    use cuenv::file_times::FileTimes;
    use cuenv::state::StateManager;
    use cuenv::sync_env::SyncEnv;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// Clean up any existing state before tests
    fn cleanup_state() {
        let vars = [
            "CUENV_DIR",
            "CUENV_FILE",
            "CUENV_DIFF",
            "CUENV_WATCHES",
            "CUENV_STATE",
            "CUENV_PREFIX",
        ];

        for var in &vars {
            let _ = SyncEnv::remove_var(var);
        }
    }

    /// Test concurrent state loading and unloading
    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_concurrent_state_transitions() {
        cleanup_state();

        let num_threads = 5;
        let iterations = 10;
        let barrier = Arc::new(Barrier::new(num_threads));
        let errors = Arc::new(std::sync::Mutex::new(Vec::new()));
        let successful_loads = Arc::new(AtomicU32::new(0));
        let successful_unloads = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let barrier = Arc::clone(&barrier);
                let errors = Arc::clone(&errors);
                let successful_loads = Arc::clone(&successful_loads);
                let successful_unloads = Arc::clone(&successful_unloads);

                thread::spawn(move || {
                    let runtime = Runtime::new().unwrap();
                    barrier.wait();

                    for i in 0..iterations {
                        let temp_dir = TempDir::new().unwrap();
                        let dir = temp_dir.path();
                        let file = dir.join("env.cue");

                        runtime.block_on(async {
                            // Create unique state
                            let diff = EnvDiff::new(
                                HashMap::from([(
                                    format!("OLD_VAR_{thread_id}"),
                                    "old".to_string(),
                                )]),
                                HashMap::from([(
                                    format!("NEW_VAR_{thread_id}"),
                                    "new".to_string(),
                                )]),
                            );
                            let watches = FileTimes::new();

                            // Try to load state
                            match StateManager::load(
                                dir,
                                &file,
                                Some(&format!("env_{thread_id}_{i}")),
                                &[format!("cap_{thread_id}")],
                                &diff,
                                &watches,
                            )
                            .await
                            {
                                Ok(_) => {
                                    successful_loads.fetch_add(1, Ordering::SeqCst);

                                    // Note: We cannot verify the specific environment loaded
                                    // because another thread may have already loaded a different state.
                                    // The important thing is that state operations complete successfully
                                    // without corrupting the state.

                                    // Small delay to increase contention
                                    thread::sleep(Duration::from_micros(50));

                                    // Try to unload
                                    match StateManager::unload().await {
                                        Ok(_) => {
                                            successful_unloads.fetch_add(1, Ordering::SeqCst);
                                        }
                                        Err(e) => {
                                            errors.lock().unwrap().push(format!(
                                                "Thread {thread_id} unload error: {e}"
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    errors
                                        .lock()
                                        .unwrap()
                                        .push(format!("Thread {thread_id} load error: {e}"));
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

        let error_list = errors.lock().unwrap();
        let loads = successful_loads.load(Ordering::SeqCst);
        let unloads = successful_unloads.load(Ordering::SeqCst);

        println!(
            "State test results - Loads: {}, Unloads: {}, Errors: {}",
            loads,
            unloads,
            error_list.len()
        );

        if !error_list.is_empty() {
            for error in error_list.iter() {
                eprintln!("Error: {error}");
            }
            panic!("State management race conditions detected");
        }

        // Verify all operations completed
        assert_eq!(loads, (num_threads * iterations) as u32);
        assert_eq!(unloads, (num_threads * iterations) as u32);
    }

    /// Test state consistency under rapid changes
    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_state_consistency() {
        cleanup_state();

        let runtime = Runtime::new().unwrap();
        let num_writers = 3;
        let num_readers = 3;
        let duration_secs = 2;
        let barrier = Arc::new(Barrier::new(num_writers + num_readers));
        let inconsistencies = Arc::new(AtomicU32::new(0));
        let start_time = std::time::Instant::now();

        // Writer threads that modify state
        let writer_handles: Vec<_> = (0..num_writers)
            .map(|writer_id| {
                let barrier = Arc::clone(&barrier);

                thread::spawn(move || {
                    let runtime = Runtime::new().unwrap();
                    barrier.wait();
                    let mut counter = 0;

                    while start_time.elapsed().as_secs() < duration_secs {
                        let temp_dir = TempDir::new().unwrap();

                        runtime.block_on(async {
                            let diff = EnvDiff::new(HashMap::new(), HashMap::new());
                            let watches = FileTimes::new();

                            StateManager::load(
                                temp_dir.path(),
                                &temp_dir.path().join("env.cue"),
                                Some(&format!("writer_{writer_id}_v{counter}")),
                                &[],
                                &diff,
                                &watches,
                            )
                            .await
                            .ok();

                            thread::sleep(Duration::from_millis(10));

                            StateManager::unload().await.ok();
                        });

                        counter += 1;
                    }
                })
            })
            .collect();

        // Reader threads that check state consistency
        let reader_handles: Vec<_> = (0..num_readers)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let inconsistencies = Arc::clone(&inconsistencies);

                thread::spawn(move || {
                    barrier.wait();

                    while start_time.elapsed().as_secs() < duration_secs {
                        // Check state consistency using atomic snapshot
                        let (is_loaded, current_dir, state) = StateManager::get_state_snapshot();

                        // Verify consistency
                        match (is_loaded, current_dir, state) {
                            (true, Some(_), Some(_)) => {
                                // Consistent: loaded state
                            }
                            (false, None, None) => {
                                // Consistent: no state
                            }
                            _ => {
                                // Inconsistent state detected
                                inconsistencies.fetch_add(1, Ordering::SeqCst);
                            }
                        }

                        thread::sleep(Duration::from_millis(5));
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
        println!("State consistency test - Inconsistencies detected: {total_inconsistencies}");

        assert_eq!(
            total_inconsistencies, 0,
            "State should remain consistent under concurrent access"
        );
    }

    /// Test environment variable synchronization
    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_env_var_sync_race_conditions() {
        cleanup_state();

        let num_threads = 10;
        let iterations = 50;
        let barrier = Arc::new(Barrier::new(num_threads));
        let errors = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let barrier = Arc::clone(&barrier);
                let errors = Arc::clone(&errors);

                thread::spawn(move || {
                    barrier.wait();

                    for i in 0..iterations {
                        let key = format!("RACE_TEST_VAR_{thread_id}_{i}");
                        let value = format!("value_{thread_id}_{i}");

                        // Set variable
                        if SyncEnv::set_var(&key, &value).is_err() {
                            errors.fetch_add(1, Ordering::SeqCst);
                            continue;
                        }

                        // Immediately read it back
                        match SyncEnv::var(&key) {
                            Ok(Some(read_value)) => {
                                if read_value != value {
                                    errors.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                            Ok(None) => {
                                errors.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(_) => {
                                errors.fetch_add(1, Ordering::SeqCst);
                            }
                        }

                        // Clean up
                        SyncEnv::remove_var(&key).ok();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let total_errors = errors.load(Ordering::SeqCst);
        assert_eq!(
            total_errors, 0,
            "Environment variable operations should be thread-safe"
        );
    }

    /// Test state prefix handling under concurrent access
    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_concurrent_prefix_changes() {
        cleanup_state();

        let runtime = Runtime::new().unwrap();
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));
        let prefix_mismatches = Arc::new(AtomicU32::new(0));

        // Use a mutex to ensure only one thread modifies prefix at a time
        let prefix_lock = Arc::new(std::sync::Mutex::new(()));

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let barrier = Arc::clone(&barrier);
                let prefix_mismatches = Arc::clone(&prefix_mismatches);
                let prefix_lock = Arc::clone(&prefix_lock);

                thread::spawn(move || {
                    let runtime = Runtime::new().unwrap();
                    barrier.wait();

                    // Acquire lock before modifying prefix
                    let _lock = prefix_lock.lock().unwrap();

                    // Set a unique prefix for this thread
                    let prefix = format!("PREFIX_{thread_id}");
                    SyncEnv::set_var("CUENV_PREFIX", &prefix).unwrap();

                    let temp_dir = TempDir::new().unwrap();

                    runtime.block_on(async {
                        let diff = EnvDiff::new(HashMap::new(), HashMap::new());
                        let watches = FileTimes::new();

                        // Load state with prefix
                        StateManager::load(
                            temp_dir.path(),
                            &temp_dir.path().join("env.cue"),
                            Some(&format!("env_{thread_id}")),
                            &[],
                            &diff,
                            &watches,
                        )
                        .await
                        .unwrap();

                        // Verify the correct prefixed variables were set
                        let expected_var = format!("{prefix}_CUENV_DIR");
                        if SyncEnv::var(&expected_var).unwrap().is_none() {
                            prefix_mismatches.fetch_add(1, Ordering::SeqCst);
                        }

                        // Clean up
                        StateManager::unload().await.unwrap();
                        SyncEnv::remove_var("CUENV_PREFIX").unwrap();
                    });

                    // Lock is automatically dropped here
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let mismatches = prefix_mismatches.load(Ordering::SeqCst);
        assert_eq!(
            mismatches, 0,
            "All prefixed state variables should be set correctly"
        );
    }
}
