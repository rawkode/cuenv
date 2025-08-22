//! Test command to try the new petgraph DAG implementation

use crate::commands::env::load_env_manager;
use cuenv_cache::CacheConfig;
use cuenv_core::Result;
use cuenv_task::TaskExecutor;

pub async fn test_petgraph_dag(task_name: &str) -> Result<()> {
    println!("Testing petgraph DAG for task: {}", task_name);

    // Load environment
    let env_manager = load_env_manager().await?;

    // Create cache config
    let cache_config = CacheConfig {
        base_dir: std::env::current_dir()?.join(".cache"),
        max_size: 1024 * 1024 * 100, // 100MB
        mode: cuenv_cache::CacheMode::ReadWrite,
        inline_threshold: 4096,
        env_filter: Default::default(),
        task_env_filters: std::collections::HashMap::new(),
    };

    // Create executor
    let executor =
        TaskExecutor::new_with_config(env_manager, std::env::current_dir()?, cache_config).await?;

    // Build petgraph DAG
    let dag = executor.build_petgraph_dag(&[task_name.to_string()])?;

    println!("✓ Built petgraph DAG successfully!");
    println!("Root task: {}", dag.get_root_task());

    // Get execution levels
    match dag.get_execution_levels() {
        Ok(levels) => {
            println!("Execution levels:");
            for (i, level) in levels.iter().enumerate() {
                println!("  Level {}: {:?}", i + 1, level);
            }
        }
        Err(e) => {
            println!("❌ Error getting execution levels: {}", e);
        }
    }

    // Check if graph is cyclic
    if dag.is_cyclic() {
        println!("❌ Graph is cyclic!");
    } else {
        println!("✓ Graph is acyclic!");
    }

    // Get task dependencies
    if let Some(deps) = dag.get_task_dependencies(task_name) {
        println!("Dependencies for {}: {:?}", task_name, deps);
    } else {
        println!("No dependencies found for {}", task_name);
    }

    Ok(())
}
