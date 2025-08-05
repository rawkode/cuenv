//! Performance benchmarks for Phase 2 cache implementation
//!
//! Measures compression effectiveness, WAL overhead, and overall performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use cuenv::cache::{Cache, ProductionCache, UnifiedCacheConfig as CacheConfig};
use tempfile::TempDir;
use tokio::runtime::Runtime;

fn setup_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn bench_compression_ratios(c: &mut Criterion) {
    let rt = setup_runtime();
    let mut group = c.benchmark_group("compression_ratios");

    // Test different data patterns
    let test_data = vec![
        ("zeros", vec![0u8; 100_000]),
        (
            "sequential",
            (0..100_000).map(|i| (i % 256) as u8).collect(),
        ),
        ("random", {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            (0..100_000)
                .map(|i| {
                    let mut hasher = DefaultHasher::new();
                    i.hash(&mut hasher);
                    (hasher.finish() % 256) as u8
                })
                .collect()
        }),
        (
            "text",
            "Lorem ipsum dolor sit amet ".repeat(4000).into_bytes(),
        ),
    ];

    for (name, data) in test_data {
        group.throughput(Throughput::Bytes(data.len() as u64));

        group.bench_with_input(BenchmarkId::new("compressed", name), &data, |b, data| {
            let data = data.clone();
            b.to_async(&rt).iter(move || {
                let data = data.clone();
                async move {
                    let temp_dir = TempDir::new().unwrap();
                    let config = CacheConfig {
                        compression_enabled: true,
                        ..Default::default()
                    };

                    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                        .await
                        .unwrap();

                    cache.put("test", &data, None).await.unwrap();
                    let _: Vec<u8> = cache.get("test").await.unwrap().unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("uncompressed", name), &data, |b, data| {
            let data = data.clone();
            b.to_async(&rt).iter(move || {
                let data = data.clone();
                async move {
                    let temp_dir = TempDir::new().unwrap();
                    let config = CacheConfig {
                        compression_enabled: false,
                        ..Default::default()
                    };

                    let cache = ProductionCache::new(temp_dir.path().to_path_buf(), config)
                        .await
                        .unwrap();

                    cache.put("test", &data, None).await.unwrap();
                    let _: Vec<u8> = cache.get("test").await.unwrap().unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_wal_overhead(c: &mut Criterion) {
    let rt = setup_runtime();
    let mut group = c.benchmark_group("wal_overhead");

    let sizes = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
    ];

    for (name, size) in sizes {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("write", name), &data, |b, data| {
            b.to_async(&rt).iter_with_setup(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let cache = rt.block_on(async {
                        ProductionCache::new(temp_dir.path().to_path_buf(), CacheConfig::default())
                            .await
                            .unwrap()
                    });
                    (temp_dir, cache)
                },
                |(_temp_dir, cache)| {
                    let data = data.clone();
                    async move {
                        cache.put("test", &data, None).await.unwrap();
                    }
                },
            );
        });
    }

    group.finish();
}

fn bench_checksum_verification(c: &mut Criterion) {
    let rt = setup_runtime();
    let mut group = c.benchmark_group("checksum_verification");

    let sizes = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
    ];

    for (name, size) in sizes {
        let data = vec![42u8; size];
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("read_with_checksum", name),
            &data,
            |b, data| {
                b.to_async(&rt).iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let temp_path = temp_dir.path().to_path_buf();
                        let data = data.clone();
                        let cache = rt.block_on(async move {
                            let c = ProductionCache::new(temp_path, CacheConfig::default())
                                .await
                                .unwrap();
                            c.put("test", &data, None).await.unwrap();
                            c
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        let _: Vec<u8> = black_box(cache.get("test").await.unwrap().unwrap());
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = setup_runtime();
    let mut group = c.benchmark_group("concurrent_operations");
    group.sample_size(10); // Reduce sample size for concurrent tests

    let thread_counts = vec![1, 2, 4, 8];

    for threads in thread_counts {
        group.bench_with_input(
            BenchmarkId::new("mixed_workload", threads),
            &threads,
            |b, &threads| {
                b.to_async(&rt).iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let cache = rt.block_on(async {
                            ProductionCache::new(
                                temp_dir.path().to_path_buf(),
                                CacheConfig::default(),
                            )
                            .await
                            .unwrap()
                        });

                        // Pre-populate some data
                        rt.block_on(async {
                            for i in 0..100 {
                                let key = format!("key_{}", i);
                                let value = format!("value_{}", i);
                                cache.put(&key, &value, None).await.unwrap();
                            }
                        });

                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        let mut handles = vec![];

                        for thread_id in 0..threads {
                            let cache_clone = cache.clone();
                            let handle = tokio::spawn(async move {
                                // Mix of reads and writes
                                for i in 0..10 {
                                    if i % 3 == 0 {
                                        // Write
                                        let key = format!("thread_{}_{}", thread_id, i);
                                        let value = format!("value_{}_{}", thread_id, i);
                                        cache_clone.put(&key, &value, None).await.unwrap();
                                    } else {
                                        // Read
                                        let key = format!("key_{}", (thread_id * 10 + i) % 100);
                                        let _: Option<String> =
                                            cache_clone.get(&key).await.unwrap();
                                    }
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_large_value_handling(c: &mut Criterion) {
    let rt = setup_runtime();
    let mut group = c.benchmark_group("large_values");
    group.sample_size(10); // Large values take longer

    let sizes = vec![
        ("1MB", 1024 * 1024),
        ("5MB", 5 * 1024 * 1024),
        ("10MB", 10 * 1024 * 1024),
    ];

    for (name, size) in sizes {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("write_compressed", name),
            &data,
            |b, data| {
                b.to_async(&rt).iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let config = CacheConfig {
                            compression_enabled: true,
                            ..Default::default()
                        };
                        let cache = rt.block_on(async {
                            ProductionCache::new(temp_dir.path().to_path_buf(), config)
                                .await
                                .unwrap()
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| {
                        let data = data.clone();
                        async move {
                            cache.put("large", &data, None).await.unwrap();
                        }
                    },
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("read_compressed", name),
            &data,
            |b, data| {
                b.to_async(&rt).iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let temp_path = temp_dir.path().to_path_buf();
                        let config = CacheConfig {
                            compression_enabled: true,
                            ..Default::default()
                        };
                        let data = data.clone();
                        let cache = rt.block_on(async move {
                            let c = ProductionCache::new(temp_path, config).await.unwrap();
                            c.put("large", &data, None).await.unwrap();
                            c
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        let _: Vec<u8> = black_box(cache.get("large").await.unwrap().unwrap());
                    },
                );
            },
        );
    }

    group.finish();
}

// Skip problematic benchmarks for now to get build working
criterion_group!(
    benches,
    bench_compression_ratios,
    bench_wal_overhead,
    bench_checksum_verification,
    bench_concurrent_operations,
    bench_large_value_handling
);
criterion_main!(benches);
