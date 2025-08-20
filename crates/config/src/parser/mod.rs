//! CUE configuration parser for environment management
//!
//! This module provides functionality to parse CUE files and extract
//! environment variables, metadata, commands, tasks, and hooks.

mod ffi;
mod processing;
mod types;
mod validation;

pub use ffi::CueParser;
pub use processing::{ParseOptions, ParseResult};
pub use types::{
    CacheEnvConfig, CommandConfig, ConfigSettings, Hook, HookConfig, HookConstraint, HookType,
    HookValue, SecurityConfig, TaskCacheConfig, TaskCollection, TaskConfig, TaskNode,
    VariableMetadata,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod field_ordering_test;
