pub mod cross_package;
pub mod executor;
pub mod registry;
pub mod staging;

pub use cross_package::{parse_reference, CrossPackageReference};
pub use executor::TaskExecutor;
pub use registry::{MonorepoTaskRegistry, RegisteredTask};
pub use staging::{DependencyStager, StagedDependency, StagingStrategy};
