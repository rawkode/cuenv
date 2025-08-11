//! Processing logic for CUE parse results

use crate::parser::ffi::CueParser;
use crate::parser::types::{
    CommandConfig, CueParseResult, Hook, HookValue, HooksConfig, TaskConfig, VariableMetadata,
};
use cuenv_core::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default)]
pub struct ParseOptions {
    pub environment: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseResult {
    pub variables: HashMap<String, String>,
    pub metadata: HashMap<String, VariableMetadata>,
    pub commands: HashMap<String, CommandConfig>,
    pub tasks: HashMap<String, TaskConfig>,
    pub hooks: HashMap<String, Vec<Hook>>,
}

/// Builds the final parse result from CUE data
pub fn build_parse_result(
    mut cue_result: CueParseResult,
    options: &ParseOptions,
) -> Result<ParseResult> {
    let final_vars = build_filtered_variables(&cue_result, options);
    let hooks = extract_hooks(cue_result.hooks);

    Ok(ParseResult {
        variables: final_vars,
        metadata: std::mem::take(&mut cue_result.metadata),
        commands: std::mem::take(&mut cue_result.commands),
        tasks: std::mem::take(&mut cue_result.tasks),
        hooks,
    })
}

/// Determines if a variable should be included based on capabilities
fn should_include_variable(
    key: &str,
    metadata: &HashMap<String, VariableMetadata>,
    capabilities: &[String],
) -> bool {
    if let Some(var_metadata) = metadata.get(key) {
        if let Some(cap) = &var_metadata.capability {
            // Variable has a capability tag, only include if it matches the filter
            capabilities.is_empty() || capabilities.contains(cap)
        } else {
            // No capability tag means always include
            true
        }
    } else {
        // No metadata means no capability tag, always include
        true
    }
}

/// Processes variables from JSON values to strings
fn process_variables(
    variables: &HashMap<String, serde_json::Value>,
    metadata: &HashMap<String, VariableMetadata>,
    capabilities: &[String],
) -> HashMap<String, String> {
    let mut result = HashMap::with_capacity(variables.len());

    for (key, val) in variables {
        if should_include_variable(key, metadata, capabilities) {
            if let Some(str_val) = CueParser::value_to_string(val) {
                result.insert(key.clone(), str_val);
            }
        }
    }

    result
}

/// Builds filtered variables with environment overrides
fn build_filtered_variables(
    cue_result: &CueParseResult,
    options: &ParseOptions,
) -> HashMap<String, String> {
    // Start with base variables
    let mut final_vars = process_variables(
        &cue_result.variables,
        &cue_result.metadata,
        &options.capabilities,
    );

    // Apply environment-specific overrides
    if let Some(env_name) = &options.environment {
        if let Some(env_vars) = cue_result.environments.get(env_name) {
            let env_overrides =
                process_variables(env_vars, &cue_result.metadata, &options.capabilities);

            // Merge environment overrides into base variables
            final_vars.extend(env_overrides);
        }
    }

    final_vars
}

/// Extracts hooks from the configuration
fn extract_hooks(hooks_config: Option<HooksConfig>) -> HashMap<String, Vec<Hook>> {
    let mut hooks = HashMap::with_capacity(2); // At most 2 hook types (onEnter, onExit)

    if let Some(config) = hooks_config {
        if let Some(on_enter) = config.on_enter {
            let hook_list = match on_enter {
                HookValue::Single(hook) => vec![*hook],
                HookValue::Multiple(hook_vec) => hook_vec,
            };
            hooks.insert("onEnter".to_string(), hook_list);
        }

        if let Some(on_exit) = config.on_exit {
            let hook_list = match on_exit {
                HookValue::Single(hook) => vec![*hook],
                HookValue::Multiple(hook_vec) => hook_vec,
            };
            hooks.insert("onExit".to_string(), hook_list);
        }
    }

    hooks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_include_variable() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "AWS_KEY".to_string(),
            VariableMetadata {
                capability: Some("aws".to_string()),
            },
        );
        metadata.insert("DB_URL".to_string(), VariableMetadata { capability: None });

        // Variable with no metadata should always be included
        assert!(should_include_variable("UNKNOWN", &metadata, &[]));
        assert!(should_include_variable(
            "UNKNOWN",
            &metadata,
            &["aws".to_string()]
        ));

        // Variable with no capability should always be included
        assert!(should_include_variable("DB_URL", &metadata, &[]));
        assert!(should_include_variable(
            "DB_URL",
            &metadata,
            &["aws".to_string()]
        ));

        // Variable with capability should respect filter
        assert!(should_include_variable("AWS_KEY", &metadata, &[])); // Empty filter includes all
        assert!(should_include_variable(
            "AWS_KEY",
            &metadata,
            &["aws".to_string()]
        )); // Matching capability
        assert!(!should_include_variable(
            "AWS_KEY",
            &metadata,
            &["gcp".to_string()]
        )); // Non-matching capability
    }
}
