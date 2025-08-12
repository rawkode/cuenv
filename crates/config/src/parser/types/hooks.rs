//! Hook configuration types

use serde::{Deserialize, Serialize};

/// Hook value can be a single hook or array of hooks
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HookValue {
    Single(Box<Hook>),
    Multiple(Vec<Hook>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HookType {
    #[default]
    OnEnter,
    OnExit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookConstraint {
    /// Check if a command is available in PATH
    CommandExists { command: String },
    /// Run a custom shell command and check if it succeeds (exit code 0)
    ShellCommand {
        command: String,
        args: Option<Vec<String>>,
    },
}

/// Hook is now just an ExecHook - a simple command execution primitive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub command: String,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub inputs: Option<Vec<String>>,
    #[serde(default)]
    pub source: Option<bool>,
    #[serde(default)]
    pub preload: Option<bool>,
}

/// Legacy hook config for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub command: String,
    pub args: Vec<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub source: Option<bool>,
    #[serde(default)]
    pub constraints: Vec<HookConstraint>,
    #[serde(skip)]
    pub hook_type: HookType,
}
