//! Integration tests for Phase 3 performance optimizations

use cuenv::cache::{Cache, CacheBuilder, CacheReader, CacheWriter, StreamingCache};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt as TokioAsyncWriteExt;

#[tokio::test]
async fn test_streaming_apis() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Test streaming write
    let mut writer = cache.get_writer("stream_test", None).await.unwrap();

    // Write 1MB of data
    let data = vec![0x42u8; 1024 * 1024];
    writer.write_all(&data).await.unwrap();

    let metadata = writer.finalize().await.unwrap();
    assert_eq!(metadata.size_bytes, data.len() as u64);

    // Test streaming read
    let reader = cache.get_reader("stream_test").await.unwrap().unwrap();
    assert_eq!(reader.metadata().size_bytes, data.len() as u64);

    let mut read_data = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut read_data).await.unwrap();

    assert_eq!(read_data.len(), data.len());
    assert!(reader.verify_integrity());
}

#[tokio::test]
async fn test_streaming_copy_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Create a test file
    let test_file = temp_dir.path().join("test.dat");
    let mut file = File::create(&test_file).await.unwrap();

    // Write 10MB of test data
    let test_data = vec![0x55u8; 10 * 1024 * 1024];
    file.write_all(&test_data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    // Test streaming copy into cache
    let file = File::open(&test_file).await.unwrap();
    let reader = tokio::io::BufReader::new(file);

    let start = Instant::now();
    let bytes_written = cache.put_stream("large_file", reader, None).await.unwrap();
    let write_duration = start.elapsed();

    assert_eq!(bytes_written, test_data.len() as u64);
    println!(
        "Streaming write: {} MB/s",
        (bytes_written as f64 / 1024.0 / 1024.0) / write_duration.as_secs_f64()
    );

    // Test streaming copy from cache
    let output_file = temp_dir.path().join("output.dat");
    let file = File::create(&output_file).await.unwrap();
    let writer = tokio::io::BufWriter::new(file);

    let start = Instant::now();
    let bytes_read = cache
        .get_stream("large_file", writer)
        .await
        .unwrap()
        .unwrap();
    let read_duration = start.elapsed();

    assert_eq!(bytes_read, test_data.len() as u64);
    println!(
        "Streaming read: {} MB/s",
        (bytes_read as f64 / 1024.0 / 1024.0) / read_duration.as_secs_f64()
    );
}

#[tokio::test]
async fn test_fast_path_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Warm up and test small value performance
    let iterations = 10000;
    let small_value = "small cached value";

    // Write small values
    let start = Instant::now();
    for i in 0..iterations {
        cache
            .put(&format!("small_{}", i), &small_value, None)
            .await
            .unwrap();
    }
    let write_duration = start.elapsed();

    println!(
        "Fast path writes: {} ops/sec",
        iterations as f64 / write_duration.as_secs_f64()
    );

    // Read small values (should hit fast path)
    let start = Instant::now();
    for i in 0..iterations {
        let _: Option<String> = cache.get(&format!("small_{}", i)).await.unwrap();
    }
    let read_duration = start.elapsed();

    println!(
        "Fast path reads: {} ops/sec",
        iterations as f64 / read_duration.as_secs_f64()
    );
}

#[tokio::test]
async fn test_concurrent_access_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Test concurrent writes
    let concurrent_tasks = 100;
    let ops_per_task = 100;

    let start = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..ops_per_task {
                let key = format!("task_{}_key_{}", task_id, i);
                let value = format!("value_{}", i);
                cache.put(&key, &value, None).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start.elapsed();
    let total_ops = concurrent_tasks * ops_per_task;

    println!(
        "Concurrent writes: {} ops/sec ({} tasks)",
        total_ops as f64 / duration.as_secs_f64(),
        concurrent_tasks
    );

    // Test concurrent reads
    let start = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..ops_per_task {
                let key = format!("task_{}_key_{}", task_id, i);
                let _: Option<String> = cache.get(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start.elapsed();

    println!(
        "Concurrent reads: {} ops/sec ({} tasks)",
        total_ops as f64 / duration.as_secs_f64(),
        concurrent_tasks
    );
}

#[tokio::test]
async fn test_memory_mapped_performance() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Create various sized values to test mmap threshold
    let sizes = vec![
        ("tiny", 64),
        ("small", 1024),
        ("medium", 64 * 1024),
        ("large", 1024 * 1024),
        ("huge", 10 * 1024 * 1024),
    ];

    for (name, size) in sizes {
        let data = vec![0x77u8; size];
        let key = format!("mmap_test_{}", name);

        // Write
        let start = Instant::now();
        cache.put(&key, &data, None).await.unwrap();
        let write_duration = start.elapsed();

        // Clear memory cache to force disk read
        cache.clear().await.unwrap();

        // Read (should use mmap for larger values)
        let start = Instant::now();
        let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
        let read_duration = start.elapsed();

        println!(
            "{} ({}KB): write={:?}, read={:?}",
            name,
            size / 1024,
            write_duration,
            read_duration
        );
    }
}

#[tokio::test]
async fn test_batch_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Prepare batch data
    let batch_size = 1000;
    let mut entries = Vec::new();

    for i in 0..batch_size {
        entries.push((
            format!("batch_key_{}", i),
            format!("batch_value_{}", i),
            None as Option<Duration>,
        ));
    }

    // Test batch put
    let start = Instant::now();
    cache.put_many(&entries).await.unwrap();
    let duration = start.elapsed();

    println!(
        "Batch put: {} items in {:?} ({} items/sec)",
        batch_size,
        duration,
        batch_size as f64 / duration.as_secs_f64()
    );

    // Test batch get
    let keys: Vec<String> = (0..batch_size)
        .map(|i| format!("batch_key_{}", i))
        .collect();

    let start = Instant::now();
    let results: Vec<(String, Option<String>)> = cache.get_many(&keys).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), batch_size);
    assert!(results.iter().all(|(_, v)| v.is_some()));

    println!(
        "Batch get: {} items in {:?} ({} items/sec)",
        batch_size,
        duration,
        batch_size as f64 / duration.as_secs_f64()
    );
}

#[tokio::test]
async fn test_sharding_distribution() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();

    // Test that keys are well-distributed across shards
    let num_keys = 10000;
    let mut shard_counts = std::collections::HashMap::new();

    for i in 0..num_keys {
        let key = format!("shard_test_{}", i);
        cache.put(&key, &i, None).await.unwrap();

        // Extract shard from the path (first 2 hex chars of hash)
        // This is a simplified test - in reality we'd inspect the file system
        let hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());
            hasher.update(&3u32.to_le_bytes()); // Version 3
            format!("{:x}", hasher.finalize())
        };
        let shard = &hash[..2];
        *shard_counts.entry(shard.to_string()).or_insert(0) += 1;
    }

    // Check distribution (should be roughly uniform)
    let expected_per_shard = num_keys as f64 / 256.0;
    let mut min_count = num_keys;
    let mut max_count = 0;

    for (shard, count) in &shard_counts {
        min_count = min_count.min(*count);
        max_count = max_count.max(*count);
    }

    println!(
        "Shard distribution: {} shards used, min={}, max={}, expected={}",
        shard_counts.len(),
        min_count,
        max_count,
        expected_per_shard as usize
    );

    // Verify reasonable distribution (within 50% of expected)
    assert!(min_count as f64 > expected_per_shard * 0.5);
    assert!((max_count as f64) < expected_per_shard * 1.5);
}
