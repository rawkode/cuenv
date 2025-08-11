use cuenv_config::{CommandConfig, CueParser, Hook, HookConfig, HookType, ParseOptions, TaskConfig};
use cuenv_core::{Error, Result};
use std::collections::HashMap;
use std::path::Path;

use super::apply::apply_merged_environment;
use super::hooks::process_sourcing_hooks;

/// Load environment with given options
pub async fn load_env_with_options(
    dir: &Path,
    environment: Option<String>,
    mut capabilities: Vec<String>,
    command: Option<&str>,
    original_env: &HashMap<String, String>,
    commands: &mut HashMap<String, CommandConfig>,
    tasks: &mut HashMap<String, TaskConfig>,
    hooks: &mut HashMap<String, HookConfig>,
    cue_vars: &mut HashMap<String, String>,
    cue_vars_metadata: &mut HashMap<String, cuenv_config::VariableMetadata>,
    sourced_env: &mut HashMap<String, String>,
) -> Result<()> {
    // First pass: load package to get command mappings
    let temp_options = ParseOptions {
        environment: environment.clone(),
        capabilities: Vec::new(), // Empty for now to get all commands
    };

    let parse_result = CueParser::eval_package_with_options(dir, "env", &temp_options)?;
    commands.extend(parse_result.commands.clone());
    tasks.extend(parse_result.tasks.clone());

    // Convert Vec<Hook> to HookConfig for compatibility with TUI architecture
    convert_hooks_to_config(&parse_result.hooks, hooks);

    // If no capabilities were specified, try to infer from the command
    infer_capabilities(command, commands, &mut capabilities);

    // Second pass: load with actual capabilities
    let options = ParseOptions {
        environment,
        capabilities,
    };

    tracing::info!(
        path = %dir.display(),
        environment = ?options.environment,
        capabilities = ?options.capabilities,
        "Loading CUE package"
    );

    // First, parse CUE package to get hooks and initial environment
    let parse_result = match CueParser::eval_package_with_options(dir, "env", &options) {
        Ok(result) => result,
        Err(e) => {
            return Err(Error::cue_parse_with_source(
                dir,
                format!("Failed to evaluate CUE package: {}", dir.display()),
                e,
            ));
        }
    };

    // Store commands, tasks and hooks
    commands.extend(parse_result.commands.clone());
    tasks.extend(parse_result.tasks.clone());
    convert_hooks_to_config(&parse_result.hooks, hooks);

    // Execute sourcing hooks first to capture additional environment variables
    let sourced_env_vars = process_sourcing_hooks(dir, &parse_result.hooks).await;

    // Store the sourced environment
    let has_sourced_env = !sourced_env_vars.is_empty();
    *sourced_env = sourced_env_vars.clone();

    // Merge CUE variables with sourced variables (CUE takes precedence)
    let mut merged_variables = sourced_env_vars;
    merged_variables.extend(parse_result.variables);

    // Store variable metadata
    cue_vars_metadata.clear();
    cue_vars_metadata.extend(parse_result.metadata);

    // Apply the merged environment
    apply_merged_environment(
        dir,
        merged_variables,
        &options,
        has_sourced_env,
        original_env,
        cue_vars,
    )
    .await
}

fn convert_hooks_to_config(
    hook_list: &HashMap<String, Vec<Hook>>,
    hooks: &mut HashMap<String, HookConfig>,
) {
    for (hook_type, hook_vec) in hook_list {
        if let Some(first_hook) = hook_vec.first() {
            let hook_config = match first_hook {
                Hook::Legacy(config) => config.clone(),
                Hook::Exec { exec, .. } => HookConfig {
                    command: exec.command.clone(),
                    args: exec.args.clone().unwrap_or_default(),
                    url: None,
                    source: exec.source,
                    constraints: vec![],
                    hook_type: if hook_type == "onEnter" {
                        HookType::OnEnter
                    } else {
                        HookType::OnExit
                    },
                },
                _ => continue, // Skip other hook types for now
            };
            hooks.insert(hook_type.clone(), hook_config);
        }
    }
}

fn infer_capabilities(
    command: Option<&str>,
    commands: &HashMap<String, CommandConfig>,
    capabilities: &mut Vec<String>,
) {
    if capabilities.is_empty() {
        if let Some(cmd) = command {
            // Look up the command in our commands configuration
            if let Some(cmd_config) = commands.get(cmd) {
                if let Some(cmd_caps) = &cmd_config.capabilities {
                    tracing::info!(
                        command = %cmd,
                        capabilities = ?cmd_caps,
                        "Inferred capabilities for command"
                    );
                    *capabilities = cmd_caps.clone();
                }
            }
        }

        if capabilities.is_empty() {
            tracing::info!(
                "No capabilities specified or inferred, will load all non-capability-tagged variables"
            );
        }
    }
}