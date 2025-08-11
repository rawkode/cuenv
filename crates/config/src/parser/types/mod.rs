//! Type definitions for CUE configuration parsing
//!
//! This module contains all the data structures used to represent
//! parsed CUE configurations.

mod cache;
mod commands;
mod hooks;
mod result;
mod security;
mod tasks;

pub use cache::{CacheEnvConfig, TaskCacheConfig};
pub use commands::CommandConfig;
pub use hooks::{
    DevenvConfig, ExecConfig, Hook, HookConfig, HookConstraint, HookType, HookValue,
    NixFlakeConfig,
};
pub(crate) use result::{CueParseResult, HooksConfig};
pub use security::SecurityConfig;
pub use tasks::TaskConfig;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableMetadata {
    pub capability: Option<String>,
}
