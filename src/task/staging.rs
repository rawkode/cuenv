use crate::core::errors::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Staging strategy for dependencies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagingStrategy {
    /// Create symbolic links (default, fastest)
    Symlink,
    /// Copy files (more compatible, slower)
    Copy,
    /// Hard links (fast, same filesystem only)
    #[allow(dead_code)]
    Hardlink,
}

/// A dependency to be staged
#[derive(Debug, Clone)]
pub struct StagedDependency {
    /// Name of the dependency (e.g., "projects:frontend:dist")
    pub name: String,
    /// Source path to the dependency output
    pub source_path: PathBuf,
    /// Optional target name (if different from source)
    pub target_name: Option<String>,
}

/// Manages staging of task dependencies in isolated environments
pub struct DependencyStager {
    /// Root directory for all staged dependencies
    staging_root: TempDir,
    /// Staging strategy to use
    strategy: StagingStrategy,
    /// Staged dependencies and their paths
    staged: HashMap<String, PathBuf>,
}

impl DependencyStager {
    /// Create a new dependency stager with default strategy (Symlink)
    pub fn new() -> Result<Self> {
        Self::with_strategy(StagingStrategy::Symlink)
    }

    /// Create a new dependency stager with specific strategy
    pub fn with_strategy(strategy: StagingStrategy) -> Result<Self> {
        let staging_root = tempfile::Builder::new()
            .prefix("cuenv-staging-")
            .tempdir()
            .map_err(|e| Error::FileSystem {
                path: PathBuf::from("/tmp"),
                operation: "create staging directory".to_string(),
                source: e.into(),
            })?;

        Ok(Self {
            staging_root,
            strategy,
            staged: HashMap::new(),
        })
    }

    /// Get the staging root directory
    pub fn staging_root(&self) -> &Path {
        self.staging_root.path()
    }

    /// Get the staging strategy
    pub fn strategy(&self) -> StagingStrategy {
        self.strategy
    }

    /// Stage a dependency and return its staged path
    pub fn stage_dependency(&mut self, dependency: &StagedDependency) -> Result<PathBuf> {
        // Check if source exists
        if !dependency.source_path.exists() {
            return Err(Error::FileSystem {
                path: dependency.source_path.clone(),
                operation: "stage dependency".to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Source path does not exist: {}", dependency.source_path.display()),
                ),
            });
        }

        // Determine target name
        let target_name = dependency.target_name.as_ref().map(|s| s.as_str()).or_else(
            || {
                dependency
                    .source_path
                    .file_name()
                    .and_then(|n| n.to_str())
            },
        );

        let target_name = target_name.ok_or_else(|| {
            Error::configuration(format!(
                "Cannot determine target name for {}",
                dependency.source_path.display()
            ))
        })?;

        // Create staging path
        // Use the dependency name as a subdirectory to avoid conflicts
        let safe_name = dependency.name.replace(':', "_");
        let staging_dir = self.staging_root.path().join(&safe_name);
        fs::create_dir_all(&staging_dir).map_err(|e| Error::FileSystem {
            path: staging_dir.clone(),
            operation: "create staging subdirectory".to_string(),
            source: e,
        })?;

        let staged_path = staging_dir.join(target_name);

        // Stage based on strategy
        match self.strategy {
            StagingStrategy::Symlink => self.stage_with_symlink(&dependency.source_path, &staged_path)?,
            StagingStrategy::Copy => self.stage_with_copy(&dependency.source_path, &staged_path)?,
            StagingStrategy::Hardlink => self.stage_with_hardlink(&dependency.source_path, &staged_path)?,
        }

        // Track staged dependency
        self.staged.insert(dependency.name.clone(), staged_path.clone());

        Ok(staged_path)
    }

    /// Stage using symbolic links
    fn stage_with_symlink(&self, source: &Path, target: &Path) -> Result<()> {
        // Get absolute source path
        let absolute_source = source.canonicalize().map_err(|e| Error::FileSystem {
            path: source.to_path_buf(),
            operation: "canonicalize source".to_string(),
            source: e,
        })?;

        // Remove target if it exists
        if target.exists() {
            if target.is_dir() {
                fs::remove_dir_all(target).map_err(|e| Error::FileSystem {
                    path: target.to_path_buf(),
                    operation: "remove existing target".to_string(),
                    source: e,
                })?;
            } else {
                fs::remove_file(target).map_err(|e| Error::FileSystem {
                    path: target.to_path_buf(),
                    operation: "remove existing target".to_string(),
                    source: e,
                })?;
            }
        }

        // Create symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&absolute_source, target).map_err(|e| Error::FileSystem {
                path: target.to_path_buf(),
                operation: format!(
                    "create symlink from {} to {}",
                    absolute_source.display(),
                    target.display()
                ),
                source: e,
            })?;
        }

        #[cfg(windows)]
        {
            if absolute_source.is_dir() {
                std::os::windows::fs::symlink_dir(&absolute_source, target).map_err(|e| Error::FileSystem {
                    path: target.to_path_buf(),
                    operation: format!(
                        "create directory symlink from {} to {}",
                        absolute_source.display(),
                        target.display()
                    ),
                    source: e,
                })?;
            } else {
                std::os::windows::fs::symlink_file(&absolute_source, target).map_err(|e| Error::FileSystem {
                    path: target.to_path_buf(),
                    operation: format!(
                        "create file symlink from {} to {}",
                        absolute_source.display(),
                        target.display()
                    ),
                    source: e,
                })?;
            }
        }

        Ok(())
    }

    /// Stage using file copy
    fn stage_with_copy(&self, source: &Path, target: &Path) -> Result<()> {
        if source.is_dir() {
            self.copy_dir_recursive(source, target)
        } else {
            fs::copy(source, target).map_err(|e| Error::FileSystem {
                path: source.to_path_buf(),
                operation: format!("copy file to {}", target.display()),
                source: e,
            })?;
            Ok(())
        }
    }

    /// Recursively copy a directory
    fn copy_dir_recursive(&self, source: &Path, target: &Path) -> Result<()> {
        fs::create_dir_all(target).map_err(|e| Error::FileSystem {
            path: target.to_path_buf(),
            operation: "create target directory".to_string(),
            source: e,
        })?;

        for entry in fs::read_dir(source).map_err(|e| Error::FileSystem {
            path: source.to_path_buf(),
            operation: "read source directory".to_string(),
            source: e,
        })? {
            let entry = entry.map_err(|e| Error::FileSystem {
                path: source.to_path_buf(),
                operation: "read directory entry".to_string(),
                source: e,
            })?;

            let source_path = entry.path();
            let target_path = target.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_dir_recursive(&source_path, &target_path)?;
            } else {
                fs::copy(&source_path, &target_path).map_err(|e| Error::FileSystem {
                    path: source_path.clone(),
                    operation: format!("copy file to {}", target_path.display()),
                    source: e,
                })?;
            }
        }

        Ok(())
    }

    /// Stage using hard links
    fn stage_with_hardlink(&self, source: &Path, target: &Path) -> Result<()> {
        if source.is_dir() {
            // Can't hardlink directories, fall back to symlink
            self.stage_with_symlink(source, target)
        } else {
            fs::hard_link(source, target).map_err(|e| Error::FileSystem {
                path: source.to_path_buf(),
                operation: format!("create hard link to {}", target.display()),
                source: e,
            })?;
            Ok(())
        }
    }

    /// Get environment variables for all staged dependencies
    pub fn get_environment_variables(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        for (name, path) in &self.staged {
            // Convert "projects:frontend:dist" to "CUENV_INPUT_PROJECTS_FRONTEND_DIST"
            let env_key = format!(
                "CUENV_INPUT_{}",
                name.to_uppercase().replace(':', "_").replace('-', "_")
            );
            env_vars.insert(env_key, path.to_string_lossy().to_string());
        }

        env_vars
    }

    /// Get staged path for a dependency
    pub fn get_staged_path(&self, name: &str) -> Option<&PathBuf> {
        self.staged.get(name)
    }

    /// Stage multiple dependencies at once
    pub fn stage_all(&mut self, dependencies: &[StagedDependency]) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for dep in dependencies {
            paths.push(self.stage_dependency(dep)?);
        }
        Ok(paths)
    }
}

// The TempDir will automatically clean up when dropped

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staging_strategy_default() {
        let stager = DependencyStager::new().unwrap();
        assert_eq!(stager.strategy(), StagingStrategy::Symlink);
    }

    #[test]
    fn test_environment_variable_formatting() {
        let stager = DependencyStager::new().unwrap();
        let mut staged = HashMap::new();
        staged.insert(
            "projects:frontend:dist".to_string(),
            PathBuf::from("/tmp/staged/dist"),
        );
        staged.insert(
            "tools-ci:output".to_string(),
            PathBuf::from("/tmp/staged/output"),
        );

        // Temporarily replace the staged map for testing
        let mut test_stager = DependencyStager {
            staging_root: stager.staging_root,
            strategy: stager.strategy,
            staged,
        };

        let env_vars = test_stager.get_environment_variables();
        assert!(env_vars.contains_key("CUENV_INPUT_PROJECTS_FRONTEND_DIST"));
        assert!(env_vars.contains_key("CUENV_INPUT_TOOLS_CI_OUTPUT"));
    }
}