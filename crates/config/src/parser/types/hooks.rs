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

/// Base execution primitive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecConfig {
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
    pub constraints: Vec<HookConstraint>,
}

/// Nix flake configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NixFlakeConfig {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub impure: Option<bool>,
}

/// Devenv configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevenvConfig {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

/// Hook types supporting the layered architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Hook {
    /// Simple nix flake format (just flake field)
    SimpleNixFlake { flake: NixFlakeConfig },
    /// Simple devenv format (just devenv field)
    SimpleDevenv { devenv: DevenvConfig },
    /// Legacy format for backward compatibility
    Legacy(HookConfig),
    /// Basic command execution with type field
    Exec {
        #[serde(rename = "type")]
        hook_type: String,
        #[serde(flatten)]
        exec: ExecConfig,
    },
    /// Nix flake integration with explicit type
    NixFlake {
        #[serde(rename = "type")]
        hook_type: String,
        #[serde(flatten)]
        exec: ExecConfig,
        flake: NixFlakeConfig,
    },
    /// Devenv integration with explicit type
    Devenv {
        #[serde(rename = "type")]
        hook_type: String,
        #[serde(flatten)]
        exec: ExecConfig,
        devenv: DevenvConfig,
    },
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
