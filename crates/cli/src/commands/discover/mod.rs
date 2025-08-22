use cuenv_config::{CueParser, ParseOptions, ParseResult};
use cuenv_core::{Error, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A discovered CUE package with its metadata
#[derive(Debug, Clone)]
pub struct DiscoveredPackage {
    /// Hierarchical name of the package (e.g., "projects:backend")
    pub name: String,
    /// Absolute path to the directory containing env.cue
    pub path: PathBuf,
    /// Path relative to the cue.mod root
    pub _relative_path: PathBuf,
    /// The parsed CUE package (if loaded)
    pub parse_result: Option<ParseResult>,
}

/// Discovery configuration and state
pub struct PackageDiscovery {
    /// Maximum depth to search for env.cue files
    max_depth: usize,
    /// The root directory containing cue.mod
    pub module_root: Option<PathBuf>,
}

impl PackageDiscovery {
    /// Create a new package discovery instance
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            module_root: None,
        }
    }

    /// Find the cue.mod root directory starting from the given path
    pub fn find_module_root(start_path: &Path) -> Result<PathBuf> {
        let mut current = if start_path.is_file() {
            start_path
                .parent()
                .ok_or_else(|| Error::configuration("Invalid file path: no parent directory"))?
        } else {
            start_path
        };

        // Walk up the directory tree looking for cue.mod
        loop {
            let cue_mod_path = current.join("cue.mod");
            if cue_mod_path.exists() && cue_mod_path.is_dir() {
                return Ok(current.to_path_buf());
            }

            current = match current.parent() {
                Some(parent) => parent,
                None => {
                    return Err(Error::configuration(
                        "No cue.mod directory found in any parent directory",
                    ))
                }
            };
        }
    }

    /// Discover all env.cue files from the module root
    pub fn discover_env_files(&mut self, start_path: &Path) -> Result<Vec<PathBuf>> {
        // Find the module root first
        let module_root = Self::find_module_root(start_path)?;
        self.module_root = Some(module_root.clone());

        let mut env_files = Vec::new();

        // Walk the directory tree from module root
        for entry in WalkDir::new(&module_root)
            .max_depth(self.max_depth)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip the cue.mod directory itself
            if path.starts_with(module_root.join("cue.mod")) {
                continue;
            }

            // Check if this is an env.cue file
            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new("env.cue")) {
                env_files.push(path.to_path_buf());
            }
        }

        // Sort for consistent ordering
        env_files.sort();

        Ok(env_files)
    }

    /// Convert a path to a hierarchical package name
    pub fn format_package_name(&self, env_file_path: &Path) -> Result<String> {
        let module_root = self
            .module_root
            .as_ref()
            .ok_or_else(|| Error::configuration("Module root not set"))?;

        // Get the directory containing the env.cue file
        let package_dir = env_file_path
            .parent()
            .ok_or_else(|| Error::configuration("Invalid env.cue path"))?;

        // Get the relative path from module root
        let relative_path = package_dir
            .strip_prefix(module_root)
            .map_err(|_| Error::configuration("env.cue file not under module root"))?;

        // Convert path components to colon-separated name
        if relative_path.as_os_str().is_empty() {
            // Root package
            Ok("root".to_string())
        } else {
            let components: Vec<&str> = relative_path
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str(),
                    _ => None,
                })
                .collect();
            Ok(components.join(":"))
        }
    }

    /// Discover all packages and optionally load them
    pub async fn discover(
        &mut self,
        start_path: &Path,
        load_packages: bool,
    ) -> Result<Vec<DiscoveredPackage>> {
        // Discover all env.cue files
        let env_files = self.discover_env_files(start_path)?;

        let mut packages = Vec::new();

        for env_file in env_files {
            let package_dir = env_file
                .parent()
                .ok_or_else(|| Error::configuration("Invalid env.cue path"))?;

            let name = self.format_package_name(&env_file)?;

            let relative_path = if let Some(ref module_root) = self.module_root {
                package_dir
                    .strip_prefix(module_root)
                    .unwrap_or(package_dir)
                    .to_path_buf()
            } else {
                PathBuf::new()
            };

            let parse_result = if load_packages {
                // Load the package using existing CUE parser
                match CueParser::eval_package_with_options(
                    package_dir,
                    cuenv_core::constants::DEFAULT_PACKAGE_NAME,
                    &ParseOptions::default(),
                ) {
                    Ok(result) => Some(result),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load package at {}: {}",
                            package_dir.display(),
                            e
                        );
                        None
                    }
                }
            } else {
                None
            };

            packages.push(DiscoveredPackage {
                name,
                path: package_dir.to_path_buf(),
                _relative_path: relative_path,
                parse_result,
            });
        }

        Ok(packages)
    }

    /// Discover and load a specific package by name
    pub async fn _load_package_by_name(
        &mut self,
        start_path: &Path,
        package_name: &str,
    ) -> Result<DiscoveredPackage> {
        let packages = self.discover(start_path, false).await?;

        let package = packages
            .into_iter()
            .find(|p| p.name == package_name)
            .ok_or_else(|| Error::configuration(format!("Package '{package_name}' not found")))?;

        // Load the package
        let parse_result = CueParser::eval_package_with_options(
            &package.path,
            cuenv_core::constants::DEFAULT_PACKAGE_NAME,
            &ParseOptions::default(),
        )?;

        Ok(DiscoveredPackage {
            name: package.name,
            path: package.path,
            _relative_path: package._relative_path,
            parse_result: Some(parse_result),
        })
    }
}

/// Convenience function to discover all packages from the current directory
pub async fn _discover_packages(load: bool) -> Result<Vec<DiscoveredPackage>> {
    let current_dir = std::env::current_dir()
        .map_err(|e| Error::configuration(format!("Failed to get current directory: {e}")))?;

    let mut discovery = PackageDiscovery::new(32);
    discovery.discover(&current_dir, load).await
}

/// Convenience function to list discovered packages
pub async fn _list_packages() -> Result<()> {
    let packages = _discover_packages(false).await?;

    if packages.is_empty() {
        tracing::info!("No CUE packages found");
        return Ok(());
    }

    tracing::info!("Discovered CUE packages:");
    for package in packages {
        tracing::info!("  {} -> {}", package.name, package.path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_module_root() {
        let temp_dir = TempDir::new().unwrap();
        let cue_mod_dir = temp_dir.path().join("cue.mod");
        fs::create_dir(&cue_mod_dir).unwrap();

        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let root = PackageDiscovery::find_module_root(&subdir).unwrap();
        assert_eq!(root, temp_dir.path());
    }

    #[test]
    fn test_format_package_name() {
        let temp_dir = TempDir::new().unwrap();
        let cue_mod_dir = temp_dir.path().join("cue.mod");
        fs::create_dir(&cue_mod_dir).unwrap();

        let mut discovery = PackageDiscovery::new(32);
        discovery.module_root = Some(temp_dir.path().to_path_buf());

        // Test root package
        let root_env = temp_dir.path().join("env.cue");
        assert_eq!(discovery.format_package_name(&root_env).unwrap(), "root");

        // Test nested package
        let nested_env = temp_dir
            .path()
            .join("projects")
            .join("backend")
            .join("env.cue");
        assert_eq!(
            discovery.format_package_name(&nested_env).unwrap(),
            "projects:backend"
        );
    }

    #[tokio::test]
    async fn test_discover_env_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create cue.mod
        let cue_mod_dir = temp_dir.path().join("cue.mod");
        fs::create_dir(&cue_mod_dir).unwrap();

        // Create env.cue files in various locations
        fs::write(temp_dir.path().join("env.cue"), "package cuenv\n").unwrap();

        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir(&projects_dir).unwrap();
        fs::write(projects_dir.join("env.cue"), "package cuenv\n").unwrap();

        let backend_dir = projects_dir.join("backend");
        fs::create_dir(&backend_dir).unwrap();
        fs::write(backend_dir.join("env.cue"), "package cuenv\n").unwrap();

        let mut discovery = PackageDiscovery::new(32);
        let env_files = discovery.discover_env_files(temp_dir.path()).unwrap();

        assert_eq!(env_files.len(), 3);
    }
}
mod execute;
pub use execute::execute;
