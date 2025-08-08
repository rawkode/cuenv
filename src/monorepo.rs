use crate::core::errors::{Error, Result};
use crate::discovery::PackageDiscovery;
use crate::env_manager::EnvManager;
use crate::task::{parse_reference, CrossPackageReference, MonorepoTaskRegistry, TaskExecutor};
use std::path::Path;

/// Execute a task in a monorepo context
pub async fn execute_monorepo_task(
    current_dir: &Path,
    task_ref: &str,
    task_args: &[String],
    audit: bool,
) -> Result<i32> {
    // Check if this is a cross-package reference
    let parsed_ref = parse_reference(task_ref)?;

    match parsed_ref {
        CrossPackageReference::LocalTask { task } => {
            // Local task - use regular execution
            execute_local_task(current_dir, &task, task_args, audit).await
        }
        _ => {
            // Cross-package task - need to discover and execute
            execute_cross_package_task(current_dir, task_ref, task_args, audit).await
        }
    }
}

/// Execute a local task in the current directory
async fn execute_local_task(
    current_dir: &Path,
    task_name: &str,
    task_args: &[String],
    audit: bool,
) -> Result<i32> {
    // Use regular env_manager for local tasks
    let mut env_manager = EnvManager::new()?;
    env_manager.load_directory(current_dir)?;

    let executor = env_manager.create_task_executor()?;

    if audit {
        executor.execute_task_with_audit(task_name, task_args).await
    } else {
        executor.execute_task(task_name, task_args).await
    }
}

/// Execute a cross-package task
async fn execute_cross_package_task(
    current_dir: &Path,
    task_ref: &str,
    _task_args: &[String],
    _audit: bool,
) -> Result<i32> {
    // Find the module root
    let mut discovery = PackageDiscovery::new(32);

    // Discover all packages in the monorepo
    let packages = discovery.discover(current_dir, true).await?;

    if packages.is_empty() {
        return Err(Error::configuration(
            "No packages found in the repository".to_string(),
        ));
    }

    // Build the task registry
    let registry = MonorepoTaskRegistry::from_packages(packages)?;

    // Validate all dependencies
    registry.validate_all_dependencies()?;

    // Create and execute using the cross-package executor
    let mut executor = TaskExecutor::new(registry);

    // Execute the task
    executor.execute(task_ref)?;

    Ok(0)
}

/// List all available tasks in the monorepo
pub async fn list_monorepo_tasks(current_dir: &Path) -> Result<()> {
    // Discover all packages
    let mut discovery = PackageDiscovery::new(32);
    let packages = discovery.discover(current_dir, true).await?;

    if packages.is_empty() {
        println!("No packages found in the repository");
        return Ok(());
    }

    // Build the task registry
    let registry = MonorepoTaskRegistry::from_packages(packages)?;

    // List all tasks
    let all_tasks = registry.list_all_tasks();

    if all_tasks.is_empty() {
        println!("No tasks found");
    } else {
        println!("Available tasks:");
        println!();

        // Group tasks by package for better readability
        let mut by_package: std::collections::HashMap<String, Vec<(String, Option<String>)>> =
            std::collections::HashMap::new();

        for (full_name, description) in all_tasks {
            // Extract package name from full task name
            let parts: Vec<&str> = full_name.split(':').collect();
            if parts.len() >= 2 {
                let package = parts[..parts.len() - 1].join(":");
                let task = parts[parts.len() - 1].to_string();
                by_package
                    .entry(package)
                    .or_insert_with(Vec::new)
                    .push((task, description));
            }
        }

        // Sort packages for consistent output
        let mut packages: Vec<_> = by_package.keys().cloned().collect();
        packages.sort();

        for package in packages {
            println!("  Package: {}", package);
            if let Some(tasks) = by_package.get(&package) {
                for (task, desc) in tasks {
                    print!("    {}:{}", package, task);
                    if let Some(description) = desc {
                        print!(": {}", description);
                    }
                    println!();
                }
            }
            println!();
        }
    }

    Ok(())
}

/// Check if we're in a monorepo context
pub fn is_monorepo(current_dir: &Path) -> bool {
    // Check for cue.mod directory
    let cue_mod = current_dir.join("cue.mod");
    if cue_mod.exists() {
        return true;
    }

    // Walk up to find cue.mod
    let mut dir = current_dir;
    while let Some(parent) = dir.parent() {
        if parent.join("cue.mod").exists() {
            return true;
        }
        dir = parent;
    }

    false
}
