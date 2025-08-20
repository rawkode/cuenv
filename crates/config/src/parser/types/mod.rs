//! Type definitions for CUE configuration parsing
//!
//! This module contains all the data structures used to represent
//! parsed CUE configurations.

mod cache;
mod commands;
mod config;
mod hooks;
mod raw;
mod result;
mod security;
mod tasks;

pub use cache::{CacheEnvConfig, TaskCacheConfig};
pub use commands::CommandConfig;
pub use config::ConfigSettings;
pub use hooks::{Hook, HookConfig, HookConstraint, HookType, HookValue};
pub(crate) use raw::RawCueResult;
pub(crate) use result::{CueParseResult, HooksConfig};
pub use security::SecurityConfig;
pub use tasks::{TaskCollection, TaskConfig, TaskNode};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableMetadata {
    pub capability: Option<String>,
}
