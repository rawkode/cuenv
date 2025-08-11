//! Security configuration types

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(rename = "restrictDisk")]
    pub restrict_disk: Option<bool>,
    #[serde(rename = "restrictNetwork")]
    pub restrict_network: Option<bool>,
    #[serde(rename = "readOnlyPaths")]
    pub read_only_paths: Option<Vec<String>>,
    #[serde(rename = "readWritePaths")]
    pub read_write_paths: Option<Vec<String>>,
    #[serde(rename = "denyPaths")]
    pub deny_paths: Option<Vec<String>>,
    #[serde(rename = "allowedHosts")]
    pub allowed_hosts: Option<Vec<String>>,
    /// Automatically infer disk restrictions from task inputs/outputs
    #[serde(rename = "inferFromInputsOutputs")]
    pub infer_from_inputs_outputs: Option<bool>,
}
