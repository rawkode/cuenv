//! Raw types for direct CUE JSON deserialization

use serde::Deserialize;
use std::collections::HashMap;

/// Raw CUE file structure as returned by the Go bridge
#[derive(Debug, Deserialize)]
pub(crate) struct RawCueResult {
    #[serde(default)]
    pub env: RawEnv,
    #[serde(default)]
    pub tasks: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub hooks: Option<RawHooks>,
    #[serde(default)]
    pub capabilities: HashMap<String, RawCapability>,
    // Catch-all for other fields including sayHello at top level
    #[serde(flatten)]
    pub _other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawEnv {
    #[serde(default)]
    pub environment: HashMap<String, HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub capabilities: HashMap<String, RawCapability>,
    #[serde(flatten)]
    pub variables: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCapability {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawHooks {
    #[serde(rename = "onEnter")]
    pub on_enter: Option<serde_json::Value>,
    #[serde(rename = "onExit")]
    pub on_exit: Option<serde_json::Value>,
}
