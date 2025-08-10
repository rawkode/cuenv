use crate::discovery::PackageDiscovery;
use cuenv_core::{Error, Result};
use cuenv_env::EnvManager;
use cuenv_task::{
    parse_reference, CrossPackageReference, DiscoveredPackage, MonorepoTaskRegistry,
    ParseResult as TaskParseResult, TaskExecutor,
};
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
    // First, check if we're in a monorepo context
    if let Ok(module_root) = PackageDiscovery::find_module_root(current_dir) {
        // Load the local environment to check if task has cross-package dependencies
        let mut env_manager = EnvManager::new();
        env_manager.load_env(current_dir).await?;

        // Check if the task has cross-package dependencies
        if let Some(task) = env_manager.get_task(task_name) {
            if let Some(deps) = &task.dependencies {
                // Check if any dependency is a cross-package reference
                for dep in deps {
                    // A cross-package dependency contains a colon
                    if dep.contains(':') {
                        // This task has cross-package dependencies, use monorepo execution
                        // We need to figure out the full package name for this directory
                        let mut discovery = PackageDiscovery::new(32);
                        let packages = discovery.discover(&module_root, true).await?;

                        // Find the package for the current directory
                        // Need to canonicalize paths for accurate comparison
                        let canonical_current = current_dir
                            .canonicalize()
                            .unwrap_or_else(|_| current_dir.to_path_buf());

                        let mut found_package = None;
                        for package in &packages {
                            let canonical_package = package
                                .path
                                .canonicalize()
                                .unwrap_or_else(|_| package.path.clone());
                            if canonical_package == canonical_current {
                                found_package = Some(package.name.clone());
                                break;
                            }
                        }

                        if let Some(package_name) = found_package {
                            let full_task_name = format!("{package_name}:{task_name}");
                            return execute_cross_package_task(
                                &module_root,
                                &full_task_name,
                                task_args,
                                audit,
                            )
                            .await;
                        } else {
                            // If we didn't find the package, it might be because we need to discover from module root
                            // This can happen when running from a subdirectory
                            return Err(Error::configuration(format!(
                                "Could not determine package name for directory: {}. Available packages: {:?}",
                                current_dir.display(),
                                packages.iter().map(|p| (&p.name, &p.path)).collect::<Vec<_>>()
                            )));
                        }
                    }
                }
            }
        }
    }

    // No cross-package dependencies, use regular execution
    let mut env_manager = EnvManager::new();
    env_manager.load_env(current_dir).await?;

    let executor = TaskExecutor::new(env_manager, current_dir.to_path_buf()).await?;

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

    // Convert CLI DiscoveredPackage to task DiscoveredPackage
    let task_packages: Vec<DiscoveredPackage> = packages
        .into_iter()
        .map(|cli_pkg| DiscoveredPackage {
            name: cli_pkg.name,
            path: cli_pkg.path,
            parse_result: cli_pkg.parse_result.map(|config_result| TaskParseResult {
                tasks: config_result.tasks,
            }),
        })
        .collect();

    // Build the task registry
    let registry = MonorepoTaskRegistry::from_packages(task_packages)?;

    // Validate all dependencies
    registry.validate_all_dependencies()?;

    // Create executor with the monorepo registry
    let mut executor = TaskExecutor::new_with_registry(registry).await?;

    // Execute the task
    executor.execute(task_ref).await?;

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

    // Convert CLI DiscoveredPackage to task DiscoveredPackage
    let task_packages: Vec<DiscoveredPackage> = packages
        .into_iter()
        .map(|cli_pkg| DiscoveredPackage {
            name: cli_pkg.name,
            path: cli_pkg.path,
            parse_result: cli_pkg.parse_result.map(|config_result| TaskParseResult {
                tasks: config_result.tasks,
            }),
        })
        .collect();

    // Build the task registry
    let registry = MonorepoTaskRegistry::from_packages(task_packages)?;

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
                    .or_default()
                    .push((task, description));
            }
        }

        // Sort packages for consistent output
        let mut packages: Vec<_> = by_package.keys().cloned().collect();
        packages.sort();

        for package in packages {
            println!("  Package: {package}");
            if let Some(tasks) = by_package.get(&package) {
                for (task, desc) in tasks {
                    print!("    {package}:{task}");
                    if let Some(description) = desc {
                        print!(": {description}");
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
