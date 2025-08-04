//! Performance regression test suite for cache system
//!
//! This benchmark suite implements comprehensive performance testing
//! to catch regressions and validate Phase 8 performance requirements.
//!
//! Run with: cargo bench --bench cache_regression_bench

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use cuenv::cache::{Cache, ProductionCache, SyncCache, UnifiedCache, UnifiedCacheConfig};
use rand::prelude::*;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Generate deterministic test data for benchmarks
fn generate_test_data(size: usize, seed: u64) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..size).map(|_| rng.gen()).collect()
}

/// Generate deterministic cache keys
fn generate_keys(count: usize, seed: u64) -> Vec<String> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|i| format!("bench_key_{}_{}", i, rng.gen::<u32>()))
        .collect()
}

/// Benchmark single-threaded cache operations with various data sizes
fn bench_cache_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_throughput");
    group.measurement_time(Duration::from_secs(15));

    // Test different data sizes
    for size in [64, 1024, 4096, 16384, 65536, 262144].iter() {
        let data = generate_test_data(*size, 42);

        group.throughput(Throughput::Bytes(*size as u64));

        // Production cache write throughput
        group.bench_with_input(BenchmarkId::new("production_write", size), size, |b, _| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let config = UnifiedCacheConfig {
                        max_memory_size: Some(100 * 1024 * 1024), // 100MB
                        compression_enabled: false,               // Disable for pure throughput
                        ..Default::default()
                    };
                    rt.block_on(async {
                        ProductionCache::new(temp_dir.path().to_path_buf(), config)
                            .await
                            .unwrap()
                    })
                },
                |cache| {
                    rt.block_on(async {
                        let key = format!(
                            "throughput_key_{}",
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos()
                        );
                        cache.put(&key, &data, None).await.unwrap();
                        black_box(key);
                    })
                },
                BatchSize::SmallInput,
            );
        });

        // Production cache read throughput (hot data)
        group.bench_with_input(
            BenchmarkId::new("production_read_hot", size),
            size,
            |b, _| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            max_memory_size: Some(100 * 1024 * 1024),
                            compression_enabled: false,
                            ..Default::default()
                        };
                        rt.block_on(async {
                            let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap();

                            // Pre-populate with hot data
                            let keys = generate_keys(100, 123);
                            for key in &keys {
                                cache.put(key, &data, None).await.unwrap();
                            }

                            (cache, keys)
                        })
                    },
                    |(cache, keys)| {
                        rt.block_on(async {
                            let key = &keys[fastrand::usize(..keys.len())];
                            let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
                            black_box(result);
                        })
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Production cache read throughput (cold data - disk reads)
        group.bench_with_input(
            BenchmarkId::new("production_read_cold", size),
            size,
            |b, _| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            max_memory_size: Some(1024), // Very small memory cache to force disk reads
                            compression_enabled: false,
                            ..Default::default()
                        };
                        rt.block_on(async {
                            let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap();

                            // Pre-populate data
                            let keys = generate_keys(10, 456);
                            for key in &keys {
                                cache.put(key, &data, None).await.unwrap();
                            }

                            // Clear memory cache to force disk reads
                            cache.clear().await.unwrap();

                            (cache, keys)
                        })
                    },
                    |(cache, keys)| {
                        rt.block_on(async {
                            let key = &keys[fastrand::usize(..keys.len())];
                            let result: Option<Vec<u8>> = cache.get(key).await.unwrap();
                            black_box(result);
                        })
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark cache operations under various concurrency levels
fn bench_cache_concurrency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_concurrency");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(20);

    for num_tasks in [1, 2, 4, 8, 16, 32].iter() {
        let data = generate_test_data(1024, 789);

        group.throughput(Throughput::Elements(*num_tasks as u64 * 100));

        // Mixed read/write workload
        group.bench_with_input(
            BenchmarkId::new("mixed_workload", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            max_memory_size: Some(50 * 1024 * 1024), // 50MB
                            max_entries: 10000,
                            compression_enabled: true, // Test with compression
                            ..Default::default()
                        };
                        rt.block_on(async {
                            let cache = Arc::new(
                                ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                    .await
                                    .unwrap(),
                            );

                            // Pre-populate some data for reads
                            let base_keys = generate_keys(1000, 999);
                            for (i, key) in base_keys.iter().enumerate().take(500) {
                                let value = generate_test_data(1024, i as u64);
                                cache.put(key, &value, None).await.unwrap();
                            }

                            (cache, base_keys)
                        })
                    },
                    |(cache, base_keys)| async move {
                        let mut handles = Vec::new();

                        for task_id in 0..num_tasks {
                            let cache_clone = Arc::clone(&cache);
                            let keys_clone = base_keys.clone();
                            let task_data = data.clone();

                            let handle = tokio::spawn(async move {
                                let mut rng = StdRng::seed_from_u64(task_id as u64);

                                for i in 0..100 {
                                    if rng.gen_bool(0.7) {
                                        // 70% reads
                                        let key = &keys_clone[rng.gen_range(0..keys_clone.len())];
                                        let _: Option<Vec<u8>> =
                                            cache_clone.get(key).await.unwrap_or(None);
                                    } else {
                                        // 30% writes
                                        let key = format!("task_{}_op_{}", task_id, i);
                                        let _ = cache_clone.put(&key, &task_data, None).await;
                                    }
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Write-heavy workload
        group.bench_with_input(
            BenchmarkId::new("write_heavy", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        rt.block_on(async {
                            Arc::new(
                                ProductionCache::new(
                                    temp_dir.path().to_path_buf(),
                                    Default::default(),
                                )
                                .await
                                .unwrap(),
                            )
                        })
                    },
                    |cache| async move {
                        let mut handles = Vec::new();

                        for task_id in 0..num_tasks {
                            let cache_clone = Arc::clone(&cache);
                            let task_data = data.clone();

                            let handle = tokio::spawn(async move {
                                for i in 0..100 {
                                    let key = format!("write_heavy_{}_{}", task_id, i);
                                    let _ = cache_clone.put(&key, &task_data, None).await;
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Read-heavy workload
        group.bench_with_input(
            BenchmarkId::new("read_heavy", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        rt.block_on(async {
                            let cache = Arc::new(
                                ProductionCache::new(
                                    temp_dir.path().to_path_buf(),
                                    Default::default(),
                                )
                                .await
                                .unwrap(),
                            );

                            // Pre-populate data
                            let keys = generate_keys(1000, 111);
                            for (i, key) in keys.iter().enumerate() {
                                let value = generate_test_data(1024, i as u64);
                                cache.put(key, &value, None).await.unwrap();
                            }

                            (cache, keys)
                        })
                    },
                    |(cache, keys)| async move {
                        let mut handles = Vec::new();

                        for task_id in 0..num_tasks {
                            let cache_clone = Arc::clone(&cache);
                            let keys_clone = keys.clone();

                            let handle = tokio::spawn(async move {
                                let mut rng = StdRng::seed_from_u64(task_id as u64);

                                for _ in 0..100 {
                                    let key = &keys_clone[rng.gen_range(0..keys_clone.len())];
                                    let _: Option<Vec<u8>> =
                                        cache_clone.get(key).await.unwrap_or(None);
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark cache eviction performance under memory pressure
fn bench_cache_eviction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_eviction");
    group.measurement_time(Duration::from_secs(10));

    // Test different eviction scenarios
    for scenario in ["lru_pressure", "size_pressure", "count_pressure"].iter() {
        group.bench_function(*scenario, |b| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let config = match *scenario {
                        "lru_pressure" => UnifiedCacheConfig {
                            max_memory_size: Some(1024 * 1024), // 1MB - will cause eviction
                            max_entries: 10000,
                            ..Default::default()
                        },
                        "size_pressure" => UnifiedCacheConfig {
                            max_memory_size: Some(100 * 1024 * 1024), // 100MB
                            max_size_bytes: 1024,                     // 1KB max entry size
                            ..Default::default()
                        },
                        "count_pressure" => UnifiedCacheConfig {
                            max_memory_size: Some(100 * 1024 * 1024),
                            max_entries: 100, // Only 100 entries allowed
                            ..Default::default()
                        },
                        _ => unreachable!(),
                    };

                    rt.block_on(async {
                        ProductionCache::new(temp_dir.path().to_path_buf(), config)
                            .await
                            .unwrap()
                    })
                },
                |cache| async {
                    match *scenario {
                        "lru_pressure" => {
                            // Fill cache beyond memory limit
                            for i in 0..2000 {
                                let key = format!("lru_key_{}", i);
                                let value = generate_test_data(1024, i as u64);
                                let _ = cache.put(&key, &value, None).await;
                            }
                        }
                        "size_pressure" => {
                            // Try to store values exceeding entry size limit
                            for i in 0..100 {
                                let key = format!("size_key_{}", i);
                                let value = generate_test_data(2048, i as u64); // Exceeds 1KB limit
                                let _ = cache.put(&key, &value, None).await;
                            }
                        }
                        "count_pressure" => {
                            // Store more entries than allowed
                            for i in 0..500 {
                                let key = format!("count_key_{}", i);
                                let value = generate_test_data(100, i as u64);
                                let _ = cache.put(&key, &value, None).await;
                            }
                        }
                        _ => unreachable!(),
                    }

                    let stats = cache.statistics().await.unwrap();
                    black_box(stats);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark cache metadata operations
fn bench_cache_metadata(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_metadata");

    for num_entries in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("metadata_scan", num_entries),
            num_entries,
            |b, &num_entries| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        rt.block_on(async {
                            let cache = ProductionCache::new(
                                temp_dir.path().to_path_buf(),
                                Default::default(),
                            )
                            .await
                            .unwrap();

                            // Pre-populate cache
                            let keys = generate_keys(num_entries, 222);
                            for (i, key) in keys.iter().enumerate() {
                                let value = generate_test_data(512, i as u64);
                                cache.put(key, &value, None).await.unwrap();
                            }

                            (cache, keys)
                        })
                    },
                    |(cache, keys)| async {
                        let mut total_size = 0u64;

                        for key in keys {
                            if let Some(metadata) = cache.metadata(&key).await.unwrap() {
                                total_size += metadata.size_bytes;
                            }
                        }

                        black_box(total_size);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    // Benchmark statistics collection
    group.bench_function("statistics", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                rt.block_on(async {
                    let cache =
                        ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                            .await
                            .unwrap();

                    // Add some data
                    for i in 0..1000 {
                        let key = format!("stats_key_{}", i);
                        let value = generate_test_data(256, i as u64);
                        cache.put(&key, &value, None).await.unwrap();
                    }

                    cache
                })
            },
            |cache| async {
                let stats = cache.statistics().await.unwrap();
                black_box(stats);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark compression and decompression performance
fn bench_cache_compression(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_compression");

    for (data_type, seed) in [
        ("random", 333),
        ("compressible", 444), // Repeated patterns
        ("text_like", 555),
    ]
    .iter()
    {
        let data = match *data_type {
            "random" => generate_test_data(8192, *seed),
            "compressible" => {
                let pattern = b"Hello World! This is a test pattern that should compress well. ";
                pattern.iter().cycle().take(8192).cloned().collect()
            }
            "text_like" => "The quick brown fox jumps over the lazy dog. "
                .repeat(100)
                .as_bytes()
                .to_vec(),
            _ => unreachable!(),
        };

        // With compression
        group.bench_with_input(
            BenchmarkId::new(format!("{}_compressed", data_type), data.len()),
            &data,
            |b, data| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            compression_enabled: true,
                            ..Default::default()
                        };
                        rt.block_on(async {
                            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap()
                        })
                    },
                    |cache| async {
                        let key = format!(
                            "compression_test_{}",
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos()
                        );
                        cache.put(&key, data, None).await.unwrap();
                        let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                        black_box(retrieved);
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // Without compression
        group.bench_with_input(
            BenchmarkId::new(format!("{}_uncompressed", data_type), data.len()),
            &data,
            |b, data| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            compression_enabled: false,
                            ..Default::default()
                        };
                        rt.block_on(async {
                            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap()
                        })
                    },
                    |cache| async {
                        let key = format!(
                            "no_compression_test_{}",
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos()
                        );
                        cache.put(&key, data, None).await.unwrap();
                        let retrieved: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                        black_box(retrieved);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark TTL (Time-To-Live) operations
fn bench_cache_ttl(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_ttl");

    // Test different TTL scenarios
    for ttl_secs in [1, 5, 30, 300].iter() {
        group.bench_with_input(
            BenchmarkId::new("ttl_expiration", ttl_secs),
            ttl_secs,
            |b, &ttl_secs| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = UnifiedCacheConfig {
                            ttl_secs: Some(Duration::from_secs(ttl_secs)),
                            ..Default::default()
                        };
                        rt.block_on(async {
                            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap()
                        })
                    },
                    |cache| async {
                        let data = generate_test_data(1024, 666);

                        // Store entries with TTL
                        for i in 0..100 {
                            let key = format!("ttl_key_{}", i);
                            cache.put(&key, &data, None).await.unwrap();
                        }

                        // Read them back immediately
                        let mut found = 0;
                        for i in 0..100 {
                            let key = format!("ttl_key_{}", i);
                            if cache.get::<Vec<u8>>(&key).await.unwrap().is_some() {
                                found += 1;
                            }
                        }

                        black_box(found);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark sync vs async cache performance
fn bench_sync_vs_async(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("sync_vs_async");
    let data = generate_test_data(1024, 777);

    // Async cache
    group.bench_function("async_cache", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                rt.block_on(async {
                    ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                        .await
                        .unwrap()
                })
            },
            |cache| async {
                for i in 0..100 {
                    let key = format!("async_key_{}", i);
                    cache.put(&key, &data, None).await.unwrap();
                    let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Sync cache
    group.bench_function("sync_cache", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let rt = tokio::runtime::Runtime::new().unwrap();
                let unified_cache = rt
                    .block_on(UnifiedCache::new(
                        temp_dir.path().to_path_buf(),
                        Default::default(),
                    ))
                    .unwrap();
                SyncCache::new(unified_cache).unwrap()
            },
            |cache| {
                for i in 0..100 {
                    let key = format!("sync_key_{}", i);
                    cache.put(&key, &data, None).unwrap();
                    let _: Option<Vec<u8>> = cache.get(&key).unwrap_or(None);
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark cache performance under error conditions
fn bench_cache_error_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_error_handling");

    // Test performance with invalid keys
    group.bench_function("invalid_keys", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                rt.block_on(async {
                    ProductionCache::new(temp_dir.path().to_path_buf(), Default::default())
                        .await
                        .unwrap()
                })
            },
            |cache| async {
                let invalid_keys = vec![
                    "",                // Empty key
                    "x".repeat(10000), // Very long key
                    "invalid\0key",    // Key with null bytes
                ];

                for key in invalid_keys {
                    let _ = cache.get::<Vec<u8>>(&key).await;
                    let _ = cache.put(&key, b"test", None).await;
                    let _ = cache.metadata(&key).await;
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Test performance with oversized values
    group.bench_function("oversized_values", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let config = UnifiedCacheConfig {
                    max_entry_size: 1024, // 1KB limit
                    ..Default::default()
                };
                rt.block_on(async {
                    ProductionCache::new(temp_dir.path().to_path_buf(), config)
                        .await
                        .unwrap()
                })
            },
            |cache| async {
                // Try to store values that exceed the limit
                for i in 0..50 {
                    let key = format!("oversized_key_{}", i);
                    let value = generate_test_data(2048, i as u64); // 2KB > 1KB limit
                    let _ = cache.put(&key, &value, None).await;
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cache_throughput,
    bench_cache_concurrency,
    bench_cache_eviction,
    bench_cache_metadata,
    bench_cache_compression,
    bench_cache_ttl,
    bench_sync_vs_async,
    bench_cache_error_handling
);
criterion_main!(benches);
