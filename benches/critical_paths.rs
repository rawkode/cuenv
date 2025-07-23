use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cuenv::types::{Capabilities, CommandArguments, EnvironmentVariables};
use std::collections::HashMap;

fn benchmark_environment_variables(c: &mut Criterion) {
    let mut group = c.benchmark_group("EnvironmentVariables");

    // Benchmark creating environment variables
    group.bench_function("new", |b| {
        b.iter(|| EnvironmentVariables::new());
    });

    // Benchmark inserting variables
    group.bench_function("insert_100", |b| {
        b.iter(|| {
            let mut env = EnvironmentVariables::new();
            for i in 0..100 {
                env.insert(format!("VAR_{}", i), format!("value_{}", i));
            }
            black_box(env)
        });
    });

    // Benchmark merging environments
    group.bench_function("merge_50_50", |b| {
        let mut env1 = EnvironmentVariables::new();
        let mut env2 = EnvironmentVariables::new();

        for i in 0..50 {
            env1.insert(format!("VAR_A_{}", i), format!("value_a_{}", i));
            env2.insert(format!("VAR_B_{}", i), format!("value_b_{}", i));
        }

        b.iter(|| {
            let mut env = env1.clone();
            env.merge(env2.clone());
            black_box(env)
        });
    });

    // Benchmark filtering
    group.bench_function("filter_prefix", |b| {
        let mut env = EnvironmentVariables::new();
        for i in 0..100 {
            env.insert(format!("PREFIX_{}", i), format!("value_{}", i));
            env.insert(format!("OTHER_{}", i), format!("value_{}", i));
        }

        b.iter(|| env.filter(|k, _| k.starts_with("PREFIX_")));
    });

    group.finish();
}

fn benchmark_command_arguments(c: &mut Criterion) {
    let mut group = c.benchmark_group("CommandArguments");

    // Benchmark creating arguments
    group.bench_function("from_vec_10", |b| {
        let args: Vec<String> = (0..10).map(|i| format!("arg_{}", i)).collect();
        b.iter(|| CommandArguments::from_vec(args.clone()));
    });

    // Benchmark extending arguments
    group.bench_function("extend_100", |b| {
        b.iter(|| {
            let mut args = CommandArguments::new();
            for i in 0..100 {
                args.push(format!("arg_{}", i));
            }
            black_box(args)
        });
    });

    group.finish();
}

fn benchmark_capabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("Capabilities");

    // Benchmark capability lookup
    group.bench_function("contains_in_100", |b| {
        let mut caps = Capabilities::new();
        for i in 0..100 {
            caps.add(format!("capability_{}", i));
        }

        b.iter(|| caps.contains("capability_50"));
    });

    // Benchmark adding capabilities with deduplication
    group.bench_function("add_dedupe", |b| {
        let mut caps = Capabilities::new();
        caps.add("existing");

        b.iter(|| {
            caps.add("existing"); // Should not add duplicate
            black_box(&caps)
        });
    });

    group.finish();
}

fn benchmark_newtype_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("NewtypeConversions");

    // Benchmark converting HashMap to EnvironmentVariables
    group.bench_function("hashmap_to_env", |b| {
        let mut map = HashMap::new();
        for i in 0..50 {
            map.insert(format!("VAR_{}", i), format!("value_{}", i));
        }

        b.iter(|| EnvironmentVariables::from_map(map.clone()));
    });

    // Benchmark converting to inner types
    group.bench_function("env_into_inner", |b| {
        let mut env = EnvironmentVariables::new();
        for i in 0..50 {
            env.insert(format!("VAR_{}", i), format!("value_{}", i));
        }

        b.iter_with_setup(|| env.clone(), |env| black_box(env.into_inner()));
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_environment_variables,
    benchmark_command_arguments,
    benchmark_capabilities,
    benchmark_newtype_conversions
);
criterion_main!(benches);
