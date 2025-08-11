//! Result types for CUE parsing

use super::{CommandConfig, HookValue, TaskConfig, VariableMetadata};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(crate) struct CueParseResult {
    pub variables: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, VariableMetadata>,
    pub environments: HashMap<String, HashMap<String, serde_json::Value>>,
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub tasks: HashMap<String, TaskConfig>,
    pub hooks: Option<HooksConfig>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HooksConfig {
    #[serde(rename = "onEnter")]
    pub on_enter: Option<HookValue>,
    #[serde(rename = "onExit")]
    pub on_exit: Option<HookValue>,
}
