pub mod cross_package;
pub mod staging;

pub use cross_package::{CrossPackageReference, parse_reference};
pub use staging::{DependencyStager, StagedDependency, StagingStrategy};