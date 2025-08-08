pub mod cross_package;
pub mod registry;
pub mod staging;

pub use cross_package::{CrossPackageReference, parse_reference};
pub use registry::{MonorepoTaskRegistry, RegisteredTask};
pub use staging::{DependencyStager, StagedDependency, StagingStrategy};