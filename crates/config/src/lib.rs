//! Configuration parsing and management for cuenv
//!
//! This crate handles parsing, caching, and centralized loading of CUE configuration files.
//!
//! # Centralized Configuration Management
//!
//! The core of this crate is the [`Config`] struct and [`ConfigLoader`] which provide
//! centralized configuration management for cuenv. Instead of loading configuration
//! throughout the application, use the `ConfigLoader` to load configuration once at
//! startup and pass the immutable `Config` to all components that need it.
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use cuenv_config::{ConfigLoader, RuntimeSettings};
//! use std::path::PathBuf;
//!
//! // Load configuration at application startup
//! let config = ConfigLoader::new()
//!     .with_directory(PathBuf::from("/path/to/project"))
//!     .with_environment("dev".to_string())
//!     .with_capabilities(vec!["web".to_string()])
//!     .with_runtime_settings(RuntimeSettings::default())
//!     .load()
//!     .expect("Failed to load configuration");
//!
//! // Pass the config to components that need it
//! let env_manager = cuenv_env::EnvManager::new(std::sync::Arc::new(config));
//! ```
//!
//! ## Migration from Direct Parsing
//!
//! Old pattern:
//! ```rust,ignore
//! let parser = CueParser::new();
//! let result = parser.eval_package_with_options(dir, "env", &options)?;
//! ```
//!
//! New pattern:
//! ```rust,no_run
//! let config = ConfigLoader::new()
//!     .with_directory(dir.to_path_buf())
//!     .load()?;
//! ```

pub mod cache;
pub mod config;
pub mod loader;
pub mod parser;

// Re-export main types for centralized configuration management
pub use config::{Config, PackageInfo, RuntimeSettings, SecurityContext};
pub use loader::{ConfigLoader, load_config_from_directory, load_config_with_capabilities};

// Re-export existing types for backward compatibility
pub use cache::*;
pub use parser::*;
