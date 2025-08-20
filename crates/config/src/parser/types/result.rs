//! Result types for CUE parsing

use super::{CommandConfig, ConfigSettings, HookValue, VariableMetadata};
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(crate) struct CueParseResult {
    pub variables: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, VariableMetadata>,
    pub environments: HashMap<String, HashMap<String, serde_json::Value>>,
    pub commands: HashMap<String, CommandConfig>,
    #[serde(default)]
    pub tasks: IndexMap<String, serde_json::Value>,
    pub hooks: Option<HooksConfig>,
    pub config: Option<ConfigSettings>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HooksConfig {
    #[serde(rename = "onEnter")]
    pub on_enter: Option<HookValue>,
    #[serde(rename = "onExit")]
    pub on_exit: Option<HookValue>,
}
