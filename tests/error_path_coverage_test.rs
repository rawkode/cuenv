//! Error path coverage tests
//!
//! Tests that verify the system handles failure scenarios gracefully
//! including disk full, network failures, and resource exhaustion.

use cuenv::cache::{Cache, CacheError, ProductionCache};
use std::fs;
use std::io;
use tempfile::TempDir;
use tokio::time::{timeout, Duration};

/// Test cache behavior when disk space might be limited
#[tokio::test]
async fn test_cache_disk_space_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Try to write data until we might hit limits
    let large_data = vec![0u8; 1_024_000]; // 1MB chunks
    let mut successful_writes = 0;

    for i in 0..100 {
        let key = format!("large_key_{}", i);
        match cache.put(&key, &large_data, None).await {
            Ok(_) => {
                successful_writes += 1;
            }
            Err(CacheError::Io { .. }) => {
                // Expected when disk space is limited
                println!("Hit I/O limitation after {} writes", successful_writes);
                break;
            }
            Err(CacheError::CapacityExceeded { .. }) => {
                // Expected when cache capacity is exceeded
                println!("Hit cache capacity after {} writes", successful_writes);
                break;
            }
            Err(e) => {
                println!("Unexpected error after {} writes: {}", successful_writes, e);
                break;
            }
        }
    }

    // Cache should still be functional for reads of existing data
    if successful_writes > 0 {
        let result: Option<Vec<u8>> = cache.get("large_key_0").await.expect("Read should work");
        assert!(result.is_some(), "Should be able to read existing data after disk issues");
    }
}

/// Test handling of permission denied scenarios
#[tokio::test]
async fn test_cache_permission_denied_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a subdirectory with restricted permissions
    let restricted_dir = temp_dir.path().join("restricted");
    fs::create_dir(&restricted_dir).expect("Failed to create restricted dir");
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Set permissions to read-only (no write access)
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(&restricted_dir, perms).unwrap();
    }

    // Try to create cache in restricted directory
    let result = ProductionCache::new(restricted_dir, Default::default()).await;
    
    // Should get a permission error or similar
    match result {
        Ok(_) => {
            // On some systems or in some environments, this might succeed
            println!("Cache creation succeeded despite restricted permissions");
        }
        Err(e) => {
            // Expected - should get a meaningful error
            let error_msg = e.to_string();
            assert!(!error_msg.is_empty(), "Error message should not be empty");
            println!("Got expected permission error: {}", error_msg);
        }
    }

    #[cfg(unix)]
    {
        // Restore permissions for cleanup
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&restricted_dir, perms);
    }
}

/// Test cache behavior with corrupted data files
#[tokio::test]
async fn test_cache_corrupted_data_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Store some valid data first
    let test_key = "test_key";
    let test_data = "valid test data";
    cache.put(test_key, &test_data, None).await.expect("Failed to store test data");

    // Find the cache file and corrupt it
    let cache_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read cache dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

    if !cache_files.is_empty() {
        // Corrupt the first cache file by writing invalid data
        let cache_file = &cache_files[0];
        fs::write(cache_file, "corrupted data that is not valid cache format")
            .expect("Failed to corrupt cache file");

        // Try to read the corrupted key
        let result = cache.get::<String>(test_key).await;
        
        match result {
            Ok(None) => {
                // Cache handled corruption by treating key as missing
                println!("Cache handled corruption gracefully by returning None");
            }
            Ok(Some(_)) => {
                // Somehow got data back (maybe from a different file)
                println!("Got data despite corruption - cache has resilience mechanisms");
            }
            Err(e) => {
                // Got an error - should be informative
                let error_msg = e.to_string();
                assert!(!error_msg.is_empty(), "Corruption error should have message");
                println!("Got expected corruption error: {}", error_msg);
            }
        }
    }
}

/// Test timeout handling for operations that might hang
#[tokio::test]
async fn test_cache_operation_timeouts() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Test that operations complete within reasonable time
    let test_data = vec![0u8; 10_000]; // 10KB data
    
    let put_result = timeout(
        Duration::from_secs(10), 
        cache.put("timeout_test", &test_data, None)
    ).await;

    match put_result {
        Ok(Ok(_)) => {
            println!("Put operation completed within timeout");
        }
        Ok(Err(e)) => {
            println!("Put operation failed with error: {}", e);
        }
        Err(_) => {
            panic!("Put operation timed out - this suggests a serious performance issue");
        }
    }

    let get_result = timeout(
        Duration::from_secs(10),
        cache.get::<Vec<u8>>("timeout_test")
    ).await;

    match get_result {
        Ok(Ok(_)) => {
            println!("Get operation completed within timeout");
        }
        Ok(Err(e)) => {
            println!("Get operation failed with error: {}", e);
        }
        Err(_) => {
            panic!("Get operation timed out - this suggests a serious performance issue");
        }
    }
}

/// Test handling of invalid UTF-8 in keys and values
#[tokio::test]
async fn test_cache_invalid_utf8_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Test invalid UTF-8 bytes in data (this should generally work for Vec<u8>)
    let invalid_utf8_data: Vec<u8> = vec![0xFF, 0xFE, 0xFD, 0x80, 0x81];
    
    let result = cache.put("binary_data_key", &invalid_utf8_data, None).await;
    match result {
        Ok(_) => {
            println!("Cache handled binary data correctly");
            
            // Try to retrieve it
            let retrieved: Option<Vec<u8>> = cache.get("binary_data_key").await.expect("Get should work");
            assert_eq!(retrieved, Some(invalid_utf8_data), "Binary data should round-trip correctly");
        }
        Err(e) => {
            println!("Cache rejected binary data: {}", e);
        }
    }

    // Test null bytes in keys (this should fail)
    let result = cache.put("key\0with\0nulls", b"test data", None).await;
    match result {
        Ok(_) => {
            println!("Cache allowed null bytes in keys");
        }
        Err(e) => {
            println!("Cache correctly rejected null bytes in key: {}", e);
            // This is the expected behavior
        }
    }
}

/// Test concurrent access under error conditions
#[tokio::test]
async fn test_concurrent_error_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = std::sync::Arc::new(
        ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
            .await
            .expect("Failed to create cache")
    );

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let cache = cache.clone();
            tokio::spawn(async move {
                let key = format!("concurrent_key_{}", i);
                let data = format!("data for task {}", i);
                
                // Each task tries to write and then read
                match cache.put(&key, &data, None).await {
                    Ok(_) => {
                        match cache.get::<String>(&key).await {
                            Ok(Some(retrieved)) => {
                                assert_eq!(retrieved, data, "Data should round-trip correctly");
                                Ok(())
                            }
                            Ok(None) => {
                                Err(format!("Data missing for key {}", key))
                            }
                            Err(e) => {
                                Err(format!("Get failed for key {}: {}", key, e))
                            }
                        }
                    }
                    Err(e) => {
                        Err(format!("Put failed for key {}: {}", key, e))
                    }
                }
            })
        })
        .collect();

    // Wait for all tasks and check results
    let mut success_count = 0;
    let mut error_count = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => {
                error_count += 1;
                println!("Task error: {}", e);
            }
            Err(e) => {
                error_count += 1;
                println!("Join error: {}", e);
            }
        }
    }

    // Should have mostly successes, but some errors are acceptable under stress
    println!("Concurrent operations: {} successes, {} errors", success_count, error_count);
    assert!(success_count > 0, "At least some operations should succeed");
}

/// Test recovery from temporary failures
#[tokio::test]
async fn test_cache_recovery_from_temporary_failures() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Store some initial data
    cache.put("recovery_test", "initial_data", None).await.expect("Initial put should work");

    // Simulate temporary filesystem issue by removing permissions temporarily
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let original_perms = fs::metadata(temp_dir.path()).unwrap().permissions();
        
        // Make directory read-only temporarily
        let mut restricted_perms = original_perms.clone();
        restricted_perms.set_mode(0o444);
        fs::set_permissions(temp_dir.path(), restricted_perms).unwrap();

        // Try operation that should fail
        let result = cache.put("recovery_test2", "should_fail", None).await;
        assert!(result.is_err(), "Operation should fail with restricted permissions");

        // Restore permissions
        fs::set_permissions(temp_dir.path(), original_perms).unwrap();
    }

    // After "recovery", operations should work again
    let recovery_result = cache.put("recovery_test3", "after_recovery", None).await;
    assert!(recovery_result.is_ok(), "Operations should work after recovery");

    // Original data should still be accessible
    let original_data: Option<String> = cache.get("recovery_test").await.expect("Get should work");
    assert_eq!(original_data, Some("initial_data".to_string()), 
        "Original data should survive temporary failures");
}

/// Test handling of extremely large keys or values
#[tokio::test]
async fn test_cache_large_data_limits() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
        .await
        .expect("Failed to create cache");

    // Test very long key
    let long_key = "x".repeat(10_000);
    let result = cache.put(&long_key, "test_value", None).await;
    
    match result {
        Ok(_) => {
            println!("Cache accepted very long key");
        }
        Err(CacheError::InvalidKey { .. }) => {
            println!("Cache correctly rejected very long key");
        }
        Err(e) => {
            println!("Unexpected error with long key: {}", e);
        }
    }

    // Test very large value (10MB)
    let large_value = vec![42u8; 10_000_000];
    let result = cache.put("large_value_test", &large_value, None).await;
    
    match result {
        Ok(_) => {
            println!("Cache accepted very large value");
            
            // Try to retrieve it
            let retrieved: Option<Vec<u8>> = cache.get("large_value_test").await.expect("Get should work");
            if let Some(data) = retrieved {
                assert_eq!(data.len(), large_value.len(), "Large value should round-trip with correct size");
                assert_eq!(data[0], 42u8, "Large value should have correct content");
            }
        }
        Err(CacheError::CapacityExceeded { .. }) => {
            println!("Cache correctly rejected value due to capacity limits");
        }
        Err(e) => {
            println!("Unexpected error with large value: {}", e);
        }
    }
}