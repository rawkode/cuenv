use super::PackageDiscovery;
use cuenv_config::Config;
use cuenv_core::Result;
use std::sync::Arc;

pub async fn execute(config: Arc<Config>, max_depth: usize, load: bool, dump: bool) -> Result<()> {
    let current_dir = &config.working_dir;
    let mut discovery = PackageDiscovery::new(max_depth);

    // If dump is requested, we need to load the packages
    let should_load = load || dump;

    match discovery.discover(current_dir, should_load).await {
        Ok(packages) => {
            if packages.is_empty() {
                tracing::info!("No CUE packages found");
            } else if dump {
                // Dump mode: show full details for each package
                for package in packages {
                    tracing::info!("═══════════════════════════════════════════════");
                    tracing::info!("Package: {}", package.name);
                    tracing::info!("Path: {}", package.path.display());

                    if let Some(ref result) = package.parse_result {
                        tracing::info!("\nEnvironment Variables:");
                        if result.variables.is_empty() {
                            tracing::info!("  (none)");
                        } else {
                            for (key, value) in &result.variables {
                                tracing::info!("  {key}: {value}");
                            }
                        }

                        tracing::info!("\nTasks:");
                        if result.tasks.is_empty() {
                            tracing::info!("  (none)");
                        } else {
                            for (name, task) in &result.tasks {
                                if let Some(desc) = &task.description {
                                    tracing::info!("  {name}: {desc}");
                                } else {
                                    tracing::info!("  {name}");
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: just list discovered packages
                tracing::info!("Discovered {} CUE packages:", packages.len());
                for package in packages {
                    tracing::info!("  • {} ({})", package.name, package.path.display());
                    if load {
                        if let Some(ref result) = package.parse_result {
                            tracing::info!("    - {} variables", result.variables.len());
                            tracing::info!("    - {} tasks", result.tasks.len());
                        }
                    }
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
