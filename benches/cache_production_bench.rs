//! Production cache benchmarks
//!
//! Run with: cargo bench --bench cache_production_bench

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use cuenv::cache::{Cache, ProductionCache, UnifiedCacheConfig};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Benchmark single-threaded write performance
fn bench_writes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_writes");
    group.measurement_time(Duration::from_secs(10));

    for size in [1, 10, 100, 1000, 10000].iter() {
        // Benchmark original cache
        group.bench_with_input(BenchmarkId::new("original", size), size, |b, _| {
            let data = vec![0u8; *size];
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    rt.block_on(async {
                        ProductionCache::new(
                            temp_dir.path().to_path_buf(),
                            UnifiedCacheConfig::default(),
                        )
                        .await
                        .unwrap()
                    })
                },
                |cache| {
                    let data = data.clone();
                    async move {
                        for i in 0..100 {
                            let key = format!("key_{}", i);
                            cache.put(&key, &data, None).await.unwrap();
                        }
                    }
                },
                BatchSize::SmallInput,
            );
        });

        // Benchmark production cache
        group.bench_with_input(BenchmarkId::new("production", size), size, |b, _| {
            let data = vec![0u8; *size];
            b.to_async(&rt).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    rt.block_on(async {
                        ProductionCache::new(
                            temp_dir.path().to_path_buf(),
                            UnifiedCacheConfig::default(),
                        )
                        .await
                        .unwrap()
                    })
                },
                |cache| {
                    let data = data.clone();
                    async move {
                        for i in 0..100 {
                            let key = format!("key_{}", i);
                            cache.put(&key, &data, None).await.unwrap();
                        }
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark single-threaded read performance
fn bench_reads(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_reads");
    group.measurement_time(Duration::from_secs(10));

    for size in [1, 10, 100, 1000, 10000].iter() {
        // Benchmark original cache (hot reads)
        group.bench_with_input(BenchmarkId::new("original_hot", size), size, |b, _| {
            let data = vec![0u8; *size];
            let rt_ref = &rt;
            b.to_async(rt_ref).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let temp_path = temp_dir.path().to_path_buf();
                    let data = data.clone();
                    let cache = rt_ref.block_on(async move {
                        let cache = ProductionCache::new(temp_path, UnifiedCacheConfig::default())
                            .await
                            .unwrap();

                        // Pre-populate cache
                        for i in 0..100 {
                            let key = format!("key_{}", i);
                            cache.put(&key, &data, None).await.unwrap();
                        }

                        cache
                    });
                    (temp_dir, cache)
                },
                |(_temp_dir, cache)| async move {
                    for i in 0..100 {
                        let key = format!("key_{}", i);
                        let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                    }
                },
                BatchSize::SmallInput,
            );
        });

        // Benchmark production cache (hot reads)
        group.bench_with_input(BenchmarkId::new("production_hot", size), size, |b, _| {
            let data = vec![0u8; *size];
            let rt_ref = &rt;
            b.to_async(rt_ref).iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let temp_path = temp_dir.path().to_path_buf();
                    let data = data.clone();
                    let cache = rt_ref.block_on(async move {
                        let cache = ProductionCache::new(temp_path, UnifiedCacheConfig::default())
                            .await
                            .unwrap();

                        // Pre-populate cache
                        for i in 0..100 {
                            let key = format!("key_{}", i);
                            cache.put(&key, &data, None).await.unwrap();
                        }

                        cache
                    });
                    (temp_dir, cache)
                },
                |(_temp_dir, cache)| async move {
                    for i in 0..100 {
                        let key = format!("key_{}", i);
                        let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                    }
                },
                BatchSize::SmallInput,
            );
        });

        // Benchmark production cache (cold reads with mmap)
        group.bench_with_input(
            BenchmarkId::new("production_cold", size),
            size,
            |b, size| {
                let data = vec![0u8; *size];
                let rt_ref = &rt;
                b.to_async(rt_ref).iter_batched(
                    move || {
                        let temp_dir = TempDir::new().unwrap();
                        let temp_path = temp_dir.path().to_path_buf();
                        let data = data.clone();
                        let cache = rt_ref.block_on(async move {
                            let cache =
                                ProductionCache::new(temp_path, UnifiedCacheConfig::default())
                                    .await
                                    .unwrap();

                            // Pre-populate cache
                            for i in 0..100 {
                                let key = format!("key_{}", i);
                                cache.put(&key, &data, None).await.unwrap();
                            }

                            // Clear memory cache to force disk reads
                            cache.clear().await.unwrap();

                            cache
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        for i in 0..100 {
                            let key = format!("key_{}", i);
                            let _: Option<Vec<u8>> = cache.get(&key).await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent performance
fn bench_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_concurrent");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    for num_tasks in [2, 4, 8, 16].iter() {
        // Benchmark original cache
        group.bench_with_input(
            BenchmarkId::new("original", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let cache = rt.block_on(async {
                            Arc::new(
                                ProductionCache::new(
                                    temp_dir.path().to_path_buf(),
                                    UnifiedCacheConfig::default(),
                                )
                                .await
                                .unwrap(),
                            )
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        let mut handles = Vec::new();

                        for task_id in 0..num_tasks {
                            let cache_clone: Arc<ProductionCache> = Arc::clone(&cache);
                            let handle = tokio::spawn(async move {
                                for i in 0..100 {
                                    let key = format!("task_{}_key_{}", task_id, i);
                                    let value = vec![task_id as u8; 100];

                                    cache_clone.put(&key, &value, None).await.unwrap();
                                    let _: Option<Vec<u8>> = cache_clone.get(&key).await.unwrap();
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

        // Benchmark production cache
        group.bench_with_input(
            BenchmarkId::new("production", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let cache = rt.block_on(async {
                            Arc::new(
                                ProductionCache::new(
                                    temp_dir.path().to_path_buf(),
                                    UnifiedCacheConfig::default(),
                                )
                                .await
                                .unwrap(),
                            )
                        });
                        (temp_dir, cache)
                    },
                    |(_temp_dir, cache)| async move {
                        let mut handles = Vec::new();

                        for task_id in 0..num_tasks {
                            let cache_clone: Arc<ProductionCache> = Arc::clone(&cache);
                            let handle = tokio::spawn(async move {
                                for i in 0..100 {
                                    let key = format!("task_{}_key_{}", task_id, i);
                                    let value = vec![task_id as u8; 100];

                                    cache_clone.put(&key, &value, None).await.unwrap();
                                    let _: Option<Vec<u8>> = cache_clone.get(&key).await.unwrap();
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

/// Benchmark metadata scanning performance
fn bench_metadata_scanning(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_metadata_scanning");

    // Benchmark production cache metadata scanning
    group.bench_function("production_scan_1000", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let cache = rt.block_on(async {
                    let cache = ProductionCache::new(
                        temp_dir.path().to_path_buf(),
                        UnifiedCacheConfig::default(),
                    )
                    .await
                    .unwrap();

                    // Pre-populate with 1000 entries
                    for i in 0..1000 {
                        let key = format!("scan_key_{}", i);
                        let value = vec![i as u8; 1000];
                        cache.put(&key, &value, None).await.unwrap();
                    }

                    cache
                });
                (temp_dir, cache)
            },
            |(_temp_dir, cache)| async move {
                let mut total_size = 0u64;
                for i in 0..1000 {
                    let key = format!("scan_key_{}", i);
                    if let Some(metadata) = cache.metadata(&key).await.unwrap() {
                        total_size += metadata.size_bytes;
                    }
                }
                black_box(total_size);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_writes,
    bench_reads,
    bench_concurrent,
    bench_metadata_scanning
);
criterion_main!(benches);
