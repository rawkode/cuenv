use crate::core::errors::{Error, Result};
use std::fmt;

/// Represents a reference to a task, potentially in another package
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossPackageReference {
    /// Local task in the same package
    LocalTask {
        task: String,
    },
    /// Task in another package
    PackageTask {
        package: String,
        task: String,
    },
    /// Specific output from a task in another package
    PackageTaskOutput {
        package: String,
        task: String,
        output: String,
    },
}

impl CrossPackageReference {
    /// Get the package name if this is a cross-package reference
    pub fn package(&self) -> Option<&str> {
        match self {
            Self::LocalTask { .. } => None,
            Self::PackageTask { package, .. } => Some(package),
            Self::PackageTaskOutput { package, .. } => Some(package),
        }
    }

    /// Get the task name
    pub fn task(&self) -> &str {
        match self {
            Self::LocalTask { task } => task,
            Self::PackageTask { task, .. } => task,
            Self::PackageTaskOutput { task, .. } => task,
        }
    }

    /// Get the output name if specified
    pub fn output(&self) -> Option<&str> {
        match self {
            Self::LocalTask { .. } => None,
            Self::PackageTask { .. } => None,
            Self::PackageTaskOutput { output, .. } => Some(output),
        }
    }

    /// Check if this is a cross-package reference
    pub fn is_cross_package(&self) -> bool {
        !matches!(self, Self::LocalTask { .. })
    }

    /// Create a local task reference
    pub fn local(task: impl Into<String>) -> Self {
        Self::LocalTask {
            task: task.into(),
        }
    }

    /// Create a package task reference
    pub fn package_task(package: impl Into<String>, task: impl Into<String>) -> Self {
        Self::PackageTask {
            package: package.into(),
            task: task.into(),
        }
    }

    /// Create a package task output reference
    pub fn package_task_output(
        package: impl Into<String>,
        task: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Self::PackageTaskOutput {
            package: package.into(),
            task: task.into(),
            output: output.into(),
        }
    }
}

impl fmt::Display for CrossPackageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalTask { task } => write!(f, "{}", task),
            Self::PackageTask { package, task } => write!(f, "{}:{}", package, task),
            Self::PackageTaskOutput {
                package,
                task,
                output,
            } => write!(f, "{}:{}:{}", package, task, output),
        }
    }
}

/// Parse a task reference string into a CrossPackageReference
///
/// Formats:
/// - "task" -> LocalTask
/// - "package:task" -> PackageTask (package can be single component)
/// - "package:sub:task" -> PackageTask (package = "package:sub")
/// - "package:sub:task:output" -> PackageTaskOutput
///
/// The parser assumes the last component is always the task name,
/// and if there's one more component after that, it's the output.
/// Everything before the task is the package name.
pub fn parse_reference(input: &str) -> Result<CrossPackageReference> {
    // Validate input
    if input.is_empty() {
        return Err(Error::configuration("Task reference cannot be empty"));
    }

    // Check for invalid characters
    if !input
        .chars()
        .all(|c| c.is_alphanumeric() || c == ':' || c == '-' || c == '_')
    {
        return Err(Error::configuration(format!(
            "Invalid characters in task reference: {}",
            input
        )));
    }

    // Split by colons
    let parts: Vec<&str> = input.split(':').collect();

    // Check for empty components
    if parts.iter().any(|p| p.is_empty()) {
        return Err(Error::configuration(format!(
            "Task reference contains empty components: {}",
            input
        )));
    }

    // Parse based on number of components
    // Strategy: We need to distinguish between:
    // - package names (which can contain colons like "projects:frontend")
    // - task names
    // - output names
    //
    // Since we can't reliably distinguish without more context, we'll use a convention:
    // - Single component: local task
    // - Two components: package:task (single-level package)
    // - Three components: Either package:task:output OR package:subpackage:task
    //   We'll treat it as package:subpackage:task (joining first two as package)
    // - Four or more: package:...:task OR package:...:task:output
    //   We need a heuristic to decide
    
    match parts.len() {
        1 => {
            // Local task reference
            Ok(CrossPackageReference::LocalTask {
                task: parts[0].to_string(),
            })
        }
        2 => {
            // Simple package:task reference
            Ok(CrossPackageReference::PackageTask {
                package: parts[0].to_string(),
                task: parts[1].to_string(),
            })
        }
        3 => {
            // This is the ambiguous case: could be package:task:output or package:subpackage:task
            // We'll use a heuristic: if the last component is a common output name, treat as output
            // Otherwise, treat as nested package
            
            let common_outputs = ["dist", "build", "out", "output", "artifacts", "bin", "lib", "target"];
            
            if common_outputs.contains(&parts[2]) {
                // Likely package:task:output
                Ok(CrossPackageReference::PackageTaskOutput {
                    package: parts[0].to_string(),
                    task: parts[1].to_string(),
                    output: parts[2].to_string(),
                })
            } else {
                // Likely package:subpackage:task
                Ok(CrossPackageReference::PackageTask {
                    package: format!("{}:{}", parts[0], parts[1]),
                    task: parts[2].to_string(),
                })
            }
        }
        n if n >= 4 => {
            // Four or more components
            // Pattern: package:subpackage:...:task OR package:subpackage:...:task:output
            // 
            // We'll check if the last component looks like an output
            let last = parts[n - 1];
            let common_outputs = ["dist", "build", "out", "output", "artifacts", "bin", "lib", "target"];
            
            if common_outputs.contains(&last) {
                // Treat as package:...:task:output
                let package = parts[0..n - 2].join(":");
                let task = parts[n - 2];
                Ok(CrossPackageReference::PackageTaskOutput {
                    package,
                    task: task.to_string(),
                    output: last.to_string(),
                })
            } else {
                // Treat as package:...:task (no output)
                let package = parts[0..n - 1].join(":");
                Ok(CrossPackageReference::PackageTask {
                    package,
                    task: last.to_string(),
                })
            }
        }
        _ => unreachable!(), // Split always returns at least 1 element
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_creation_helpers() {
        let local = CrossPackageReference::local("build");
        assert_eq!(local.task(), "build");
        assert!(!local.is_cross_package());

        let package = CrossPackageReference::package_task("frontend", "test");
        assert_eq!(package.package(), Some("frontend"));
        assert_eq!(package.task(), "test");
        assert!(package.is_cross_package());

        let output = CrossPackageReference::package_task_output("backend", "build", "dist");
        assert_eq!(output.package(), Some("backend"));
        assert_eq!(output.task(), "build");
        assert_eq!(output.output(), Some("dist"));
        assert!(output.is_cross_package());
    }
}