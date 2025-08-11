use super::context::TaskExecutionContext;
use super::runner;
use cuenv_cache::config::{CacheConfig, CacheConfiguration};
use cuenv_core::{Result, TaskDefinition};

/// Create cache config struct from configuration
pub fn create_cache_config_struct(cache_config: &CacheConfiguration) -> Result<CacheConfig> {
    // TODO: Fix when CacheConfig is properly exposed
    let mut config = CacheConfig::default();

    // Apply global configuration
    if let Some(base_dir) = &cache_config.global.base_dir {
        config.base_dir = base_dir.clone();
    }

    if let Some(max_size) = cache_config.global.max_size {
        config.max_size = max_size;
    }

    if let Some(inline_threshold) = cache_config.global.inline_threshold {
        config.inline_threshold = inline_threshold;
    }

    if let Some(env_filter) = &cache_config.global.env_filter {
        config.env_filter = env_filter.clone();
    }

    config.mode = cache_config.global.mode;

    Ok(config)
}

/// Execute a single task with caching support
pub async fn execute_single_task_with_cache(
    ctx: &TaskExecutionContext<'_>,
    task_name: &str,
    task_definition: &TaskDefinition,
    args: &[String],
) -> Result<i32> {
    // Check if caching is enabled for this task using the new configuration system
    // TODO: Add CacheConfigResolver when moved to workspace
    let cache_enabled = false;
    let _unused = (&ctx.cache_config.global, &task_definition.cache, task_name);

    if !cache_enabled {
        // Execute without caching
        // TODO: Add tracing when moved to workspace
        // task_progress(task_name, None, "Executing task (cache disabled)");
        return runner::execute_single_task(
            task_name,
            task_definition,
            ctx.working_dir,
            args,
            ctx.audit_mode,
            ctx.capture_output,
        )
        .await;
    }

    // Generate action digest using ActionCache
    let env_vars = std::env::vars().collect();
    let digest = ctx
        .action_cache
        .compute_digest(task_name, task_definition, ctx.working_dir, env_vars)
        .await?;

    // Execute with ActionCache
    let result = ctx
        .action_cache
        .execute_action(&digest, || async {
            // TODO: Add tracing when moved to workspace
            // cache_event(task_name, false, "task_result");
            // TODO: Add tracing when moved to workspace
            // task_progress(task_name, Some(0), "Starting task execution");

            let exit_code = runner::execute_single_task(
                task_name,
                task_definition,
                ctx.working_dir,
                args,
                ctx.audit_mode,
                ctx.capture_output,
            )
            .await?;

            // Create ActionResult for caching
            // TODO: Fix when ActionResult is properly exposed
            Ok(cuenv_cache::concurrent::action::ActionResult {
                exit_code,
                stdout_hash: None, // Not captured in current implementation
                stderr_hash: None, // Not captured in current implementation
                output_files: std::collections::HashMap::new(),
                executed_at: std::time::SystemTime::now(),
                duration_ms: 0, // Not tracked in current implementation
            })
        })
        .await?;

    // Update cache manager statistics for backward compatibility
    if result.exit_code == 0 {
        // TODO: Add tracing when moved to workspace
        // task_progress(task_name, Some(100), "Task completed successfully");
        tracing::info!(task_name = %task_name, "Task completed successfully");
    } else {
        tracing::error!(
            task_name = %task_name,
            exit_code = %result.exit_code,
            "Task failed with exit code"
        );
    }

    Ok(result.exit_code)
}
