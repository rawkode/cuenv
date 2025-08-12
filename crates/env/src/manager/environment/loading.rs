use cuenv_config::{
    CommandConfig, CueParser, Hook, HookConfig, HookType, ParseOptions, TaskConfig, TaskNode,
    VariableMetadata,
};
use cuenv_core::{constants::DEFAULT_PACKAGE_NAME, Error, Result};
use std::collections::HashMap;
use std::path::Path;

use super::apply::apply_merged_environment;
use super::hooks::process_sourcing_hooks;

/// Context for loading environment with all the mutable maps
pub struct LoadEnvironmentContext<'a> {
    pub commands: &'a mut HashMap<String, CommandConfig>,
    pub tasks: &'a mut HashMap<String, TaskConfig>,
    pub task_nodes: &'a mut HashMap<String, TaskNode>,
    pub hooks: &'a mut HashMap<String, HookConfig>,
    pub cue_vars: &'a mut HashMap<String, String>,
    pub cue_vars_metadata: &'a mut HashMap<String, VariableMetadata>,
    pub sourced_env: &'a mut HashMap<String, String>,
}

/// Load environment with given options
pub async fn load_env_with_options(
    dir: &Path,
    environment: Option<String>,
    mut capabilities: Vec<String>,
    command: Option<&str>,
    original_env: &HashMap<String, String>,
    context: &mut LoadEnvironmentContext<'_>,
) -> Result<()> {
    // First pass: load package to get command mappings
    let temp_options = ParseOptions {
        environment: environment.clone(),
        capabilities: Vec::new(), // Empty for now to get all commands
    };

    let parse_result =
        CueParser::eval_package_with_options(dir, DEFAULT_PACKAGE_NAME, &temp_options)?;
    context.commands.extend(parse_result.commands.clone());
    context.tasks.extend(parse_result.tasks.clone());
    context.task_nodes.extend(parse_result.task_nodes.clone());

    // Convert Vec<Hook> to HookConfig for compatibility with TUI architecture
    convert_hooks_to_config(&parse_result.hooks, context.hooks);

    // If no capabilities were specified, try to infer from the command
    infer_capabilities(command, context.commands, &mut capabilities);

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
    let parse_result =
        match CueParser::eval_package_with_options(dir, DEFAULT_PACKAGE_NAME, &options) {
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
    context.commands.extend(parse_result.commands.clone());
    context.tasks.extend(parse_result.tasks.clone());
    context.task_nodes.extend(parse_result.task_nodes.clone());
    convert_hooks_to_config(&parse_result.hooks, context.hooks);

    // Execute sourcing hooks first to capture additional environment variables
    let sourced_env_vars = process_sourcing_hooks(dir, &parse_result.hooks).await;

    // Store the sourced environment
    let has_sourced_env = !sourced_env_vars.is_empty();
    *context.sourced_env = sourced_env_vars.clone();

    // Merge CUE variables with sourced variables (CUE takes precedence)
    let mut merged_variables = sourced_env_vars;
    merged_variables.extend(parse_result.variables);

    // Store variable metadata
    context.cue_vars_metadata.clear();
    context.cue_vars_metadata.extend(parse_result.metadata);

    // Apply the merged environment
    apply_merged_environment(
        dir,
        merged_variables,
        &options,
        has_sourced_env,
        original_env,
        context.cue_vars,
    )
    .await
}

fn convert_hooks_to_config(
    hook_list: &HashMap<String, Vec<Hook>>,
    hooks: &mut HashMap<String, HookConfig>,
) {
    for (hook_type, hook_vec) in hook_list {
        if let Some(first_hook) = hook_vec.first() {
            // All hooks are now simple ExecHooks
            let hook_config = HookConfig {
                command: first_hook.command.clone(),
                args: first_hook.args.clone().unwrap_or_default(),
                url: None,
                source: first_hook.source,
                constraints: vec![],
                hook_type: if hook_type == "onEnter" {
                    HookType::OnEnter
                } else {
                    HookType::OnExit
                },
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
