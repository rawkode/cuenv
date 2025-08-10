use super::PackageDiscovery;
use cuenv_config::Config;
use cuenv_core::Result;
use std::sync::Arc;

pub async fn execute(config: Arc<Config>, max_depth: usize, load: bool, dump: bool) -> Result<()> {
    let current_dir = &config.working_dir;
    let mut discovery = PackageDiscovery::new(max_depth);

    // If dump is requested, we need to load the packages
    let should_load = load || dump;

    match discovery.discover(&current_dir, should_load).await {
        Ok(packages) => {
            if packages.is_empty() {
                println!("No CUE packages found");
            } else if dump {
                // Dump mode: show full details for each package
                for package in packages {
                    println!("═══════════════════════════════════════════════");
                    println!("Package: {}", package.name);
                    println!("Path: {}", package.path.display());

                    if let Some(ref result) = package.parse_result {
                        println!("\nEnvironment Variables:");
                        if result.variables.is_empty() {
                            println!("  (none)");
                        } else {
                            for (key, value) in &result.variables {
                                println!("  {key}: {value}");
                            }
                        }

                        println!("\nTasks:");
                        if result.tasks.is_empty() {
                            println!("  (none)");
                        } else {
                            for (name, task) in &result.tasks {
                                if let Some(desc) = &task.description {
                                    println!("  {name}: {desc}");
                                } else {
                                    println!("  {name}");
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: just list discovered packages
                println!("Discovered {} CUE packages:", packages.len());
                for package in packages {
                    println!("  • {} ({})", package.name, package.path.display());
                    if load {
                        if let Some(ref result) = package.parse_result {
                            println!("    - {} variables", result.variables.len());
                            println!("    - {} tasks", result.tasks.len());
                        }
                    }
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
