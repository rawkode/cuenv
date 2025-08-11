//! Dependency resolution events

use serde::{Deserialize, Serialize};

/// Dependency resolution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyEvent {
    /// Dependency resolved successfully
    DependencyResolved {
        task_name: String,
        dependency_name: String,
        package_name: Option<String>,
    },
    /// Dependency resolution failed
    DependencyResolutionFailed {
        task_name: String,
        dependency_name: String,
        error: String,
    },
}
