use cuenv_core::{Error, Result};
use std::fmt;

/// Represents a reference to a task, potentially in another package
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossPackageReference {
    /// Local task in the same package
    LocalTask { task: String },
    /// Task in another package
    PackageTask { package: String, task: String },
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
        Self::LocalTask { task: task.into() }
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
            Self::LocalTask { task } => write!(f, "{task}"),
            Self::PackageTask { package, task } => write!(f, "{package}:{task}"),
            Self::PackageTaskOutput {
                package,
                task,
                output,
            } => write!(f, "{package}:{task}#{output}"),
        }
    }
}

/// Parse a task reference string into a CrossPackageReference
///
/// Formats:
/// - "task" -> LocalTask
/// - "package:task" -> PackageTask (package can be single component)
/// - "package:sub:task" -> PackageTask (package = "package:sub")
/// - "package:sub:task#output" -> PackageTaskOutput with output
///
/// The # separator is used to distinguish outputs from task/package names
pub fn parse_reference(input: &str) -> Result<CrossPackageReference> {
    // Validate input
    if input.is_empty() {
        return Err(Error::configuration("Task reference cannot be empty"));
    }

    // Check for invalid characters
    // Allow alphanumeric, colons, hyphens, underscores, slashes, dots, and # for outputs
    if !input.chars().all(|c| {
        c.is_alphanumeric() || c == ':' || c == '-' || c == '_' || c == '/' || c == '.' || c == '#'
    }) {
        return Err(Error::configuration(format!(
            "Invalid characters in task reference: {input}"
        )));
    }

    // First, check if there's an output specifier (#)
    let (task_ref, output) = if let Some(hash_pos) = input.find('#') {
        let task_part = &input[..hash_pos];
        let output_part = &input[hash_pos + 1..];

        if task_part.is_empty() {
            return Err(Error::configuration(
                "Task reference before # cannot be empty",
            ));
        }
        if output_part.is_empty() {
            return Err(Error::configuration("Output path after # cannot be empty"));
        }

        (task_part, Some(output_part))
    } else {
        (input, None)
    };

    // Split task reference by colons
    let parts: Vec<&str> = task_ref.split(':').collect();

    // Check for empty components
    if parts.iter().any(|p| p.is_empty()) {
        return Err(Error::configuration(format!(
            "Task reference contains empty components: {input}"
        )));
    }

    // Now parse based on the presence of output and number of components
    // With the # separator, we can unambiguously determine the structure:
    // - Everything before # is the task reference
    // - Everything after # is the output path
    // The task reference follows the pattern:
    // - Single component: local task
    // - Two or more components: package:...:task (last is task, rest is package)

    match (parts.len(), output) {
        // Local task without output
        (1, None) => Ok(CrossPackageReference::LocalTask {
            task: parts[0].to_string(),
        }),

        // Local task with output (rare but valid: "build#dist")
        (1, Some(_out)) => {
            // For local tasks with output, we don't have a separate variant,
            // so we return an error or could extend the enum
            Err(Error::configuration(
                "Local task output references are not supported. Use package:task#output format",
            ))
        }

        // Package:task without output
        (2, None) => Ok(CrossPackageReference::PackageTask {
            package: parts[0].to_string(),
            task: parts[1].to_string(),
        }),

        // Package:task with output
        (2, Some(out)) => Ok(CrossPackageReference::PackageTaskOutput {
            package: parts[0].to_string(),
            task: parts[1].to_string(),
            output: out.to_string(),
        }),

        // Multiple colons - everything except the last is package, last is task
        (n, None) if n >= 3 => {
            let task = parts[n - 1].to_string();
            let package = parts[..n - 1].join(":");
            Ok(CrossPackageReference::PackageTask { package, task })
        }

        // Multiple colons with output
        (n, Some(out)) if n >= 3 => {
            let task = parts[n - 1].to_string();
            let package = parts[..n - 1].join(":");
            Ok(CrossPackageReference::PackageTaskOutput {
                package,
                task,
                output: out.to_string(),
            })
        }

        _ => Err(Error::configuration(format!(
            "Invalid task reference format: {input}"
        ))),
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
