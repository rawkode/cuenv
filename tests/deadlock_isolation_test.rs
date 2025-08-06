//! Test to isolate the deadlock issue
//!
//! This test mimics the exact pattern from invariant_error_handling_safety
//! to identify where the deadlock occurs.

#[cfg(test)]
mod deadlock_tests {
    use cuenv::cache::{Cache, CacheError, ProductionCache};
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_error_recovery_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // First, try an operation that will fail (empty key)
        let empty_key = "";
        let value = vec![1, 2, 3];

        let result = cache.put(empty_key, &value, None).await;
        assert!(matches!(result, Err(CacheError::InvalidKey { .. })));
        println!("Empty key rejected successfully");

        // Now try the recovery pattern exactly as in the failing test
        let recovery_key = "recovery_after_empty_key";
        let recovery_value = b"recovery_test".to_vec();

        println!("Attempting recovery put...");

        // Add timeout to detect if this hangs
        let put_result = tokio::time::timeout(
            Duration::from_secs(5),
            cache.put(&recovery_key, &recovery_value, None),
        )
        .await;

        match put_result {
            Ok(Ok(())) => println!("Recovery put succeeded"),
            Ok(Err(e)) => panic!("Recovery put failed: {}", e),
            Err(_) => panic!("Recovery put timed out - DEADLOCK DETECTED"),
        }

        println!("Attempting recovery get...");

        // Try to get the value back
        let get_result =
            tokio::time::timeout(Duration::from_secs(5), cache.get::<Vec<u8>>(&recovery_key)).await;

        match get_result {
            Ok(Ok(Some(retrieved))) => {
                assert_eq!(retrieved, recovery_value);
                println!("Recovery get succeeded");
            }
            Ok(Ok(None)) => panic!("Recovery get returned None"),
            Ok(Err(e)) => panic!("Recovery get failed: {}", e),
            Err(_) => panic!("Recovery get timed out - DEADLOCK DETECTED"),
        }

        println!("Test completed successfully!");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_error_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .unwrap();

        // Test concurrent operations after an error
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let cache = cache.clone();
                tokio::spawn(async move {
                    // First cause an error
                    let _ = cache.put("", &vec![i], None).await;

                    // Then try recovery
                    let key = format!("recovery_{}", i);
                    match tokio::time::timeout(
                        Duration::from_secs(2),
                        cache.put(&key, &vec![i], None),
                    )
                    .await
                    {
                        Ok(Ok(())) => println!("Thread {} recovery succeeded", i),
                        Ok(Err(e)) => println!("Thread {} recovery failed: {}", i, e),
                        Err(_) => println!("Thread {} recovery TIMED OUT", i),
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = tokio::time::timeout(Duration::from_secs(10), handle).await;
        }

        println!("Concurrent test completed");
    }
}
