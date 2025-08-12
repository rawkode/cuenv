use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[derive(Default)]
pub struct ConfigSettings {
    #[serde(rename = "outputFormat")]
    pub output_format: Option<String>,

    #[serde(rename = "cacheMode")]
    pub cache_mode: Option<String>,

    #[serde(rename = "cacheEnabled")]
    pub cache_enabled: Option<bool>,

    #[serde(rename = "auditMode")]
    pub audit_mode: Option<bool>,

    #[serde(rename = "traceOutput")]
    pub trace_output: Option<bool>,

    #[serde(rename = "defaultEnvironment")]
    pub default_environment: Option<String>,

    #[serde(rename = "defaultCapabilities")]
    pub default_capabilities: Option<Vec<String>>,
}


impl ConfigSettings {
    pub fn validate(&self) -> Result<(), String> {
        // Validate output format
        if let Some(ref format) = self.output_format {
            match format.as_str() {
                "tui" | "spinner" | "simple" | "tree" => {}
                _ => {
                    return Err(format!(
                        "Invalid output format: '{format}'. Must be one of: tui, spinner, simple, tree"
                    ))
                }
            }
        }

        // Validate cache mode
        if let Some(ref mode) = self.cache_mode {
            match mode.as_str() {
                "off" | "read" | "read-write" | "write" => {}
                _ => {
                    return Err(format!(
                        "Invalid cache mode: '{mode}'. Must be one of: off, read, read-write, write"
                    ))
                }
            }
        }

        Ok(())
    }
}
