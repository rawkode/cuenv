use cuenv_core::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct ResolverConfig {
    cmd: String,
    args: Vec<String>,
}

/// Resolve secret values that may contain special resolver references
pub fn resolve_secret(value: &str) -> Result<String> {
    if let Some(json_str) = value.strip_prefix("cuenv-resolver://") {
        if let Ok(config) = serde_json::from_str::<ResolverConfig>(json_str) {
            // Execute the resolver command
            let output = std::process::Command::new(&config.cmd)
                .args(&config.args)
                .output()
                .map_err(|e| {
                    Error::configuration(format!(
                        "Failed to execute resolver command '{}': {}",
                        config.cmd, e
                    ))
                })?;

            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(result)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(Error::configuration(format!(
                    "Resolver command '{}' failed: {}",
                    config.cmd, stderr
                )))
            }
        } else {
            // If it's not valid JSON, just return the original value
            Ok(value.to_string())
        }
    } else {
        // Not a resolver reference, return as-is
        Ok(value.to_string())
    }
}