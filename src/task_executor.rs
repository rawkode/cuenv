use crate::cache::{
    ActionCache, CacheConfigLoader, CacheConfigResolver, CacheConfiguration, CacheManager,
};
use crate::config::TaskConfig;
use crate::core::errors::{Error, Result};
use crate::env::manager::EnvManager;
use crate::security::SecurityValidator;
use crate::tracing::{
    self, cache_event, level_span, pipeline_span, task_completed, task_progress, task_span,
};
use crate::utils::cleanup::handler::ProcessGuard;
use ::tracing::Instrument;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// Context for task execution to reduce function parameter count
struct TaskExecutionContext<'a> {
    cache_config: &'a CacheConfiguration,
    working_dir: &'a Path,
    action_cache: &'a ActionCache,
    audit_mode: bool,
    capture_output: bool,
}

/// Represents a task execution plan with resolved dependencies
#[derive(Debug, Clone)]
pub struct TaskExecutionPlan {
    /// Tasks organized by execution level (level 0 = no dependencies, etc.)
    pub levels: Vec<Vec<String>>,
    /// Original task configurations
    pub tasks: HashMap<String, TaskConfig>,
}

/// Main task executor that handles dependency resolution and execution
#[derive(Clone)]
pub struct TaskExecutor {
    env_manager: EnvManager,
    working_dir: PathBuf,
    cache_manager: Arc<CacheManager>,
    action_cache: Arc<ActionCache>,
    cache_config: CacheConfiguration,
}

impl TaskExecutor {
    /// Create a new task executor
    pub async fn new(env_manager: EnvManager, working_dir: PathBuf) -> Result<Self> {
        let cache_config = CacheConfigLoader::load()?;
        let cache_config_struct = Self::create_cache_config_struct(&cache_config)?;
        let mut cache_manager = CacheManager::new(cache_config_struct).await?;

        // Apply task-specific cache environment configurations
        let tasks = env_manager.get_tasks();
        cache_manager.apply_task_configs(tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        Ok(Self {
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config,
        })
    }

    /// Create a new task executor with custom cache config (for testing)
    #[cfg(test)]
    pub async fn new_with_config(
        env_manager: EnvManager,
        working_dir: PathBuf,
        cache_config: crate::cache::CacheConfig,
    ) -> Result<Self> {
        let cache_configuration = CacheConfiguration::default();
        let mut cache_manager = CacheManager::new(cache_config).await?;

        // Apply task-specific cache environment configurations
        let tasks = env_manager.get_tasks();
        cache_manager.apply_task_configs(tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        Ok(Self {
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config: cache_configuration,
        })
    }

    /// Execute a single task by name
    pub async fn execute_task(&self, task_name: &str, args: &[String]) -> Result<i32> {
        self.execute_tasks_with_dependencies(&[task_name.to_string()], args, false)
            .await
    }

    /// Execute a single task by name with audit mode
    pub async fn execute_task_with_audit(&self, task_name: &str, args: &[String]) -> Result<i32> {
        self.execute_tasks_with_dependencies(&[task_name.to_string()], args, true)
            .await
    }

    /// Execute multiple tasks with their dependencies
    pub async fn execute_tasks_with_dependencies(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
    ) -> Result<i32> {
        self.execute_tasks_with_dependencies_internal(task_names, args, audit_mode, false)
            .await
    }

    /// Internal method that supports output capture for TUI mode
    pub async fn execute_tasks_with_dependencies_internal(
        &self,
        task_names: &[String],
        args: &[String],
        audit_mode: bool,
        capture_output: bool,
    ) -> Result<i32> {
        // Build execution plan
        let plan = self.build_execution_plan(task_names)?;

        // Create pipeline span for the entire execution
        let _pipeline_span = pipeline_span(plan.tasks.len());
        let pipeline_guard = _pipeline_span.enter();

        tracing::info!(
            requested_tasks = ?task_names,
            total_tasks = %plan.tasks.len(),
            levels = %plan.levels.len(),
            "Starting task execution pipeline"
        );

        // Execute tasks level by level
        for (level_idx, level) in plan.levels.iter().enumerate() {
            let _level_span = level_span(level_idx, level.len());
            let level_guard = _level_span.enter();

            tracing::info!(
                level = %level_idx,
                tasks = ?level,
                "Starting execution level"
            );
            let mut join_set = JoinSet::new();
            let failed_tasks = Arc::new(Mutex::new(Vec::with_capacity(level.len())));

            // Launch all tasks in this level concurrently
            for task_name in level {
                let task_config = match plan.tasks.get(task_name) {
                    Some(config) => config.clone(),
                    None => {
                        return Err(Error::configuration(format!(
                            "Task '{task_name}' not found in execution plan"
                        )));
                    }
                };
                let working_dir = self.working_dir.clone();
                let task_args = args.to_vec();
                let failed_tasks = Arc::clone(&failed_tasks);
                let task_name_owned = task_name.clone();
                let action_cache = Arc::clone(&self.action_cache);
                let env_manager_clone = self.env_manager.clone();
                let cache_config = self.cache_config.clone();

                // Create task span
                let task_span = task_span(&task_name_owned, task_config.working_dir.as_deref());

                join_set.spawn(
                    async move {
                        let start_time = Instant::now();

                        // Publish task started event
                        if let Some(event_bus) = crate::tui::event_bus::EventBus::global() {
                            event_bus
                                .publish(crate::tui::events::TaskEvent::Started {
                                    task_name: task_name_owned.clone(),
                                    timestamp: start_time,
                                })
                                .await;

                            // Send task configuration info as progress messages
                            // Show capabilities for this task's command
                            if let Some(cmd) = &task_config.command {
                                let capabilities = env_manager_clone.get_command_capabilities(cmd);
                                if !capabilities.is_empty() {
                                    event_bus
                                        .publish(crate::tui::events::TaskEvent::Progress {
                                            task_name: task_name_owned.clone(),
                                            message: format!(
                                                "Capabilities: {}",
                                                capabilities.join(", ")
                                            ),
                                        })
                                        .await;
                                }
                            }

                            if let Some(shell) = &task_config.shell {
                                event_bus
                                    .publish(crate::tui::events::TaskEvent::Progress {
                                        task_name: task_name_owned.clone(),
                                        message: format!("Shell: {}", shell),
                                    })
                                    .await;
                            }

                            if let Some(timeout) = task_config.timeout {
                                event_bus
                                    .publish(crate::tui::events::TaskEvent::Progress {
                                        task_name: task_name_owned.clone(),
                                        message: format!("Timeout: {}s", timeout),
                                    })
                                    .await;
                            }

                            if task_config.cache.as_ref().is_some_and(|c| match c {
                                crate::cache::TaskCacheConfig::Simple(enabled) => *enabled,
                                crate::cache::TaskCacheConfig::Advanced { enabled, .. } => *enabled,
                            }) {
                                event_bus
                                    .publish(crate::tui::events::TaskEvent::Progress {
                                        task_name: task_name_owned.clone(),
                                        message: "Cache: enabled".to_string(),
                                    })
                                    .await;
                            }

                            if let Some(working_dir) = &task_config.working_dir {
                                event_bus
                                    .publish(crate::tui::events::TaskEvent::Progress {
                                        task_name: task_name_owned.clone(),
                                        message: format!("Working dir: {}", working_dir),
                                    })
                                    .await;
                            }

                            if let Some(security) = &task_config.security {
                                let mut restrictions = Vec::new();
                                if security.restrict_disk.unwrap_or(false) {
                                    restrictions.push("disk");
                                }
                                if security.restrict_network.unwrap_or(false) {
                                    restrictions.push("network");
                                }
                                if !restrictions.is_empty() {
                                    event_bus
                                        .publish(crate::tui::events::TaskEvent::Progress {
                                            task_name: task_name_owned.clone(),
                                            message: format!(
                                                "Security: {} restricted",
                                                restrictions.join(", ")
                                            ),
                                        })
                                        .await;
                                }
                            }
                        }

                        let ctx = TaskExecutionContext {
                            cache_config: &cache_config,
                            working_dir: &working_dir,
                            action_cache: &action_cache,
                            audit_mode,
                            capture_output,
                        };
                        match Self::execute_single_task_with_cache(
                            &ctx,
                            &task_name_owned,
                            &task_config,
                            &task_args,
                        )
                        .await
                        {
                            Ok(status) => {
                                let duration_ms = start_time.elapsed().as_millis() as u64;
                                if status != 0 {
                                    if let Ok(mut guard) = failed_tasks.lock() {
                                        guard.push((task_name_owned.clone(), status));
                                    } else {
                                        tracing::error!(
                                            "Failed to acquire lock for failed tasks tracking"
                                        );
                                    }

                                    // Publish task failed event
                                    if let Some(event_bus) =
                                        crate::tui::event_bus::EventBus::global()
                                    {
                                        event_bus
                                            .publish(crate::tui::events::TaskEvent::Failed {
                                                task_name: task_name_owned.clone(),
                                                error: format!("Task exited with code {}", status),
                                                duration_ms,
                                            })
                                            .await;
                                    }

                                    task_completed(&task_name_owned, duration_ms, false);
                                } else {
                                    // Publish task completed event
                                    if let Some(event_bus) =
                                        crate::tui::event_bus::EventBus::global()
                                    {
                                        event_bus
                                            .publish(crate::tui::events::TaskEvent::Completed {
                                                task_name: task_name_owned.clone(),
                                                exit_code: status,
                                                duration_ms,
                                            })
                                            .await;
                                    }

                                    task_completed(&task_name_owned, duration_ms, true);
                                }
                                status
                            }
                            Err(e) => {
                                let duration_ms = start_time.elapsed().as_millis() as u64;
                                if let Ok(mut guard) = failed_tasks.lock() {
                                    guard.push((task_name_owned.clone(), -1));
                                } else {
                                    tracing::error!(
                                        "Failed to acquire lock for failed tasks tracking"
                                    );
                                }

                                // Publish task failed event
                                if let Some(event_bus) = crate::tui::event_bus::EventBus::global() {
                                    event_bus
                                        .publish(crate::tui::events::TaskEvent::Failed {
                                            task_name: task_name_owned.clone(),
                                            error: e.to_string(),
                                            duration_ms,
                                        })
                                        .await;
                                }

                                tracing::error!(
                                    task_name = %task_name_owned,
                                    error = %e,
                                    "Task execution failed"
                                );
                                task_completed(&task_name_owned, duration_ms, false);
                                -1
                            }
                        }
                    }
                    .instrument(task_span),
                );
            }

            // Wait for all tasks in this level to complete
            while let Some(result) = join_set.join_next().await {
                if let Err(e) = result {
                    return Err(Error::configuration(format!("Task execution failed: {e}")));
                }
            }

            // Check if any tasks failed
            let failed = failed_tasks
                .lock()
                .map_err(|e| Error::configuration(format!("Failed to acquire lock: {e}")))?;
            if !failed.is_empty() {
                let failed_names: Vec<&str> =
                    failed.iter().map(|(name, _)| name.as_str()).collect();
                return Err(Error::configuration(format!(
                    "Tasks failed: {}",
                    failed_names.join(", ")
                )));
            }

            drop(level_guard);
        }

        drop(pipeline_guard);
        tracing::info!("Task execution pipeline completed successfully");
        Ok(0)
    }

    /// Build an execution plan with dependency resolution
    pub fn build_execution_plan(&self, task_names: &[String]) -> Result<TaskExecutionPlan> {
        let all_tasks = self.env_manager.get_tasks();

        // Validate that all requested tasks exist
        for task_name in task_names {
            if !all_tasks.contains_key(task_name) {
                return Err(Error::configuration(format!(
                    "Task '{task_name}' not found"
                )));
            }
        }

        // Build dependency graph
        let mut task_dependencies = HashMap::with_capacity(all_tasks.len());
        let mut visited = HashSet::with_capacity(all_tasks.len());
        let mut stack = HashSet::new();

        for task_name in task_names {
            Self::collect_dependencies(
                task_name,
                all_tasks,
                &mut task_dependencies,
                &mut visited,
                &mut stack,
            )?;
        }

        // Topological sort to determine execution order
        let levels = self.topological_sort(&task_dependencies)?;

        // Build final execution plan
        let mut plan_tasks = HashMap::with_capacity(task_dependencies.len());
        for task_name in task_dependencies.keys() {
            if let Some(config) = all_tasks.get(task_name) {
                plan_tasks.insert(task_name.clone(), config.clone());
            }
        }

        Ok(TaskExecutionPlan {
            levels,
            tasks: plan_tasks,
        })
    }

    /// Recursively collect all dependencies for a task
    fn collect_dependencies(
        task_name: &str,
        all_tasks: &HashMap<String, TaskConfig>,
        task_dependencies: &mut HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
    ) -> Result<()> {
        // Check for circular dependencies
        if stack.contains(task_name) {
            return Err(Error::configuration(format!(
                "Circular dependency detected involving task '{task_name}'"
            )));
        }

        if visited.contains(task_name) {
            return Ok(());
        }

        stack.insert(task_name.to_owned());

        let task_config = all_tasks
            .get(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

        let dependencies = task_config.dependencies.clone().unwrap_or_default();

        // Validate and collect dependencies
        for dep_name in &dependencies {
            if !all_tasks.contains_key(dep_name) {
                return Err(Error::configuration(format!(
                    "Dependency '{dep_name}' of task '{task_name}' not found"
                )));
            }

            Self::collect_dependencies(dep_name, all_tasks, task_dependencies, visited, stack)?;
        }

        task_dependencies.insert(task_name.to_owned(), dependencies);
        visited.insert(task_name.to_owned());
        stack.remove(task_name);

        Ok(())
    }

    /// Perform topological sort to determine execution levels
    fn topological_sort(
        &self,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<Vec<String>>> {
        let mut in_degree = HashMap::with_capacity(dependencies.len());
        let mut graph = HashMap::with_capacity(dependencies.len());

        // Initialize in-degree count and adjacency list
        for (task, deps) in dependencies {
            in_degree.entry(task.clone()).or_insert(0);
            graph.entry(task.clone()).or_insert_with(Vec::new);

            for dep in deps {
                *in_degree.entry(dep.clone()).or_insert(0) += 0; // Ensure dep is in map
                graph
                    .entry(dep.clone())
                    .or_insert_with(Vec::new)
                    .push(task.clone());
                if let Some(degree) = in_degree.get_mut(task) {
                    *degree += 1;
                } else {
                    return Err(Error::configuration(format!(
                        "Task '{task}' not found in in-degree map"
                    )));
                }
            }
        }

        let mut levels = Vec::with_capacity(dependencies.len() / 2); // Estimate
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(task, _)| task.clone())
            .collect();

        while !queue.is_empty() {
            let current_level: Vec<String> = queue.drain(..).collect();

            if current_level.is_empty() {
                break;
            }

            for task in &current_level {
                if let Some(dependents) = graph.get(task) {
                    for dependent in dependents {
                        if let Some(degree) = in_degree.get_mut(dependent) {
                            *degree -= 1;
                            if *degree == 0 {
                                queue.push_back(dependent.clone());
                            }
                        }
                    }
                }
            }

            levels.push(current_level);
        }

        // Check for remaining tasks (would indicate circular dependencies)
        let processed_count: usize = levels.iter().map(|level| level.len()).sum();
        if processed_count != dependencies.len() {
            return Err(Error::configuration(
                "Circular dependency detected in task graph".to_string(),
            ));
        }

        Ok(levels)
    }

    /// Create cache config struct from configuration
    fn create_cache_config_struct(
        cache_config: &CacheConfiguration,
    ) -> Result<crate::cache::CacheConfig> {
        let mut config = crate::cache::CacheConfig::default();

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
    async fn execute_single_task_with_cache(
        ctx: &TaskExecutionContext<'_>,
        task_name: &str,
        task_config: &TaskConfig,
        args: &[String],
    ) -> Result<i32> {
        // Check if caching is enabled for this task using the new configuration system
        let cache_enabled = CacheConfigResolver::should_cache_task(
            &ctx.cache_config.global,
            task_config.cache.as_ref(),
            task_name,
        );

        if !cache_enabled {
            // Execute without caching
            task_progress(task_name, None, "Executing task (cache disabled)");
            return Self::execute_single_task(
                task_name,
                task_config,
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
            .compute_digest(task_name, task_config, ctx.working_dir, env_vars)
            .await?;

        // Execute with ActionCache
        let result = ctx
            .action_cache
            .execute_action(&digest, || async {
                cache_event(task_name, false, "task_result");
                task_progress(task_name, Some(0), "Starting task execution");

                let exit_code = Self::execute_single_task(
                    task_name,
                    task_config,
                    ctx.working_dir,
                    args,
                    ctx.audit_mode,
                    ctx.capture_output,
                )
                .await?;

                // Create ActionResult for caching
                Ok(crate::cache::ActionResult {
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
            task_progress(task_name, Some(100), "Task completed successfully");
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

    /// Execute a single task
    async fn execute_single_task(
        task_name: &str,
        task_config: &TaskConfig,
        working_dir: &Path,
        args: &[String],
        audit_mode: bool,
        capture_output: bool,
    ) -> Result<i32> {
        // Determine what to execute
        let (shell, script_content) = match (&task_config.command, &task_config.script) {
            (Some(command), None) => {
                // Add user args to the command
                let full_command = if args.is_empty() {
                    command.clone()
                } else {
                    format!("{} {}", command, args.join(" "))
                };
                (
                    task_config
                        .shell
                        .clone()
                        .unwrap_or_else(|| "sh".to_string()),
                    full_command,
                )
            }
            (None, Some(script)) => (
                task_config
                    .shell
                    .clone()
                    .unwrap_or_else(|| "sh".to_string()),
                script.clone(),
            ),
            (Some(_), Some(_)) => {
                return Err(Error::configuration(
                    "Task cannot have both 'command' and 'script' defined".to_string(),
                ));
            }
            (None, None) => {
                return Err(Error::configuration(
                    "Task must have either 'command' or 'script' defined".to_string(),
                ));
            }
        };

        // Validate shell command for security
        // Use a static set for allowed shells to avoid repeated allocations
        static ALLOWED_SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "pwsh", "powershell"];
        let allowed_shells: HashSet<String> =
            ALLOWED_SHELLS.iter().map(|&s| s.to_string()).collect();

        SecurityValidator::validate_command(&shell, &allowed_shells)?;

        // Validate script content for dangerous patterns
        SecurityValidator::validate_shell_expansion(&script_content)?;

        // Validate user arguments
        if !args.is_empty() {
            SecurityValidator::validate_command_args(args)?;
        }

        // Determine working directory
        let exec_dir = if let Some(task_wd) = &task_config.working_dir {
            let mut dir = working_dir.to_path_buf();
            dir.push(task_wd);

            // Validate the working directory path
            SecurityValidator::validate_path(&dir, &[working_dir.to_path_buf()])?;

            dir
        } else {
            working_dir.to_path_buf()
        };

        // Configure process group for better cleanup
        let mut cmd = Command::new(&shell);
        cmd.arg("-c").arg(&script_content).current_dir(&exec_dir);

        if capture_output {
            // Capture output for TUI mode to prevent interference
            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        } else {
            // Normal mode - inherit stdio
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }

        // On Unix, create a new process group for better cleanup
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);

            // Apply resource limits before spawning
            unsafe {
                cmd.pre_exec(|| {
                    use crate::utils::limits::apply_default_limits;
                    match apply_default_limits() {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            tracing::warn!("Failed to apply resource limits: {}", e);
                            Ok(()) // Continue anyway
                        }
                    }
                });
            }
        }

        // Apply security restrictions if configured
        if let Some(security) = &task_config.security {
            use crate::access_restrictions::AccessRestrictions;
            let mut restrictions =
                AccessRestrictions::from_security_config_with_task(security, task_config);

            if audit_mode {
                restrictions.enable_audit_mode();
                task_progress(task_name, None, "Running task in audit mode...");

                let (exit_code, audit_report) = restrictions.run_with_audit(&mut cmd)?;
                audit_report.print_summary();
                return Ok(exit_code);
            } else if restrictions.has_any_restrictions() {
                restrictions.apply_to_command(&mut cmd)?;
            }
        }

        // Spawn the process with timeout
        let mut child = cmd.spawn().map_err(|e| {
            Error::command_execution(
                &shell,
                vec!["-c".to_string(), script_content.clone()],
                format!("Failed to spawn task: {e}"),
                None,
            )
        })?;

        // Handle output streaming if capturing
        let (stdout_handle, stderr_handle) = if capture_output {
            use std::io::{BufRead, BufReader};

            // Take stdout and stderr from child
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let task_name_stdout = task_name.to_string();
            let task_name_stderr = task_name.to_string();

            // Spawn thread to read stdout
            let stdout_handle = stdout.map(|stdout| {
                std::thread::spawn(move || {
                    // Create a tokio runtime for this thread
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();

                    if let Ok(rt) = rt {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(|result| result.ok()) {
                            // Try to publish to event bus
                            if let Some(event_bus) = crate::tui::event_bus::EventBus::global() {
                                let task_name = task_name_stdout.clone();
                                let event = crate::tui::events::TaskEvent::Log {
                                    task_name,
                                    stream: crate::tui::events::LogStream::Stdout,
                                    content: line,
                                };

                                // Use spawn instead of block_on to avoid blocking the runtime
                                let event_bus_clone = event_bus.clone();
                                rt.spawn(async move {
                                    event_bus_clone.publish(event).await;
                                });
                            }
                        }
                    }
                })
            });

            // Spawn thread to read stderr
            let stderr_handle = stderr.map(|stderr| {
                std::thread::spawn(move || {
                    // Create a tokio runtime for this thread
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();

                    if let Ok(rt) = rt {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines().map_while(|result| result.ok()) {
                            // Try to publish to event bus
                            if let Some(event_bus) = crate::tui::event_bus::EventBus::global() {
                                let task_name = task_name_stderr.clone();
                                let event = crate::tui::events::TaskEvent::Log {
                                    task_name,
                                    stream: crate::tui::events::LogStream::Stderr,
                                    content: line,
                                };

                                // Use spawn instead of block_on to avoid blocking the runtime
                                let event_bus_clone = event_bus.clone();
                                rt.spawn(async move {
                                    event_bus_clone.publish(event).await;
                                });
                            }
                        }
                    }
                })
            });

            (stdout_handle, stderr_handle)
        } else {
            (None, None)
        };

        // Use ProcessGuard for automatic cleanup
        let timeout = match task_config.timeout {
            Some(timeout_secs) => Duration::from_secs(timeout_secs as u64),
            None => Duration::from_secs(3600), // Default 1 hour timeout
        };

        let mut guard = ProcessGuard::new(child, timeout);

        // Wait for completion with timeout
        let status = guard.wait_with_timeout().map_err(|e| {
            Error::command_execution(
                &shell,
                vec!["-c".to_string(), script_content.clone()],
                e.to_string(),
                None,
            )
        })?;

        // Wait for output threads to complete
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        Ok(status.code().unwrap_or(1))
    }

    /// List all available tasks
    pub fn list_tasks(&self) -> Vec<(String, Option<String>)> {
        self.env_manager.list_tasks()
    }

    /// Get CUE environment variables
    pub fn get_env_vars(&self) -> &HashMap<String, String> {
        self.env_manager.get_cue_vars()
    }

    /// Get filtered environment variables for a specific task
    pub fn get_task_env_vars(&self, task_name: &str) -> HashMap<String, String> {
        // Get the task config
        let all_tasks = self.env_manager.get_tasks();
        let task_config = match all_tasks.get(task_name) {
            Some(config) => config,
            None => return HashMap::new(),
        };

        // Get the command from the task
        let command = match &task_config.command {
            Some(cmd) => cmd,
            None => return self.env_manager.get_cue_vars().clone(), // No command, return all vars
        };

        // Get capabilities for this command
        let capabilities = self.env_manager.get_command_capabilities(command);

        // Return filtered variables based on capabilities
        self.env_manager.get_filtered_vars(&capabilities)
    }

    /// Clear the task cache
    pub fn clear_cache(&self) -> Result<()> {
        self.cache_manager.clear_cache()
    }

    /// Get cache statistics
    pub fn get_cache_statistics(&self) -> Result<crate::cache::CacheStatistics> {
        Ok(self.cache_manager.get_statistics())
    }

    /// Print cache statistics
    pub fn print_cache_statistics(&self) -> Result<()> {
        let stats = self.cache_manager.get_statistics();
        println!("Cache Statistics:");
        println!("  Hits: {}", stats.hits);
        println!("  Misses: {}", stats.misses);
        println!("  Writes: {}", stats.writes);
        println!("  Errors: {}", stats.errors);
        println!("  Lock contentions: {}", stats.lock_contentions);
        println!("  Total bytes saved: {}", stats.total_bytes_saved);
        if let Some(last_cleanup) = stats.last_cleanup {
            println!("  Last cleanup: {last_cleanup:?}");
        }
        Ok(())
    }

    /// Clean up stale cache entries
    pub fn cleanup_cache(&self, _max_age: Duration) -> Result<(usize, u64)> {
        self.cache_manager.cleanup_stale_entries()?;
        Ok((0, 0)) // Return dummy values for now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    async fn create_test_env_manager_with_tasks(tasks_cue: &str) -> (EnvManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, tasks_cue).unwrap();

        let mut manager = EnvManager::new();
        manager.load_env(temp_dir.path()).await.unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_simple_task_discovery() {
        let tasks_cue = r#"package env

env: {
    DATABASE_URL: "test"
}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let tasks = executor.list_tasks();
        assert_eq!(tasks.len(), 2);

        let task_names: Vec<&String> = tasks.iter().map(|(name, _)| name).collect();
        assert!(task_names.contains(&&"build".to_string()));
        assert!(task_names.contains(&&"test".to_string()));
    }

    #[tokio::test]
    async fn test_task_dependency_resolution() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        description: "Build the project"
        command: "echo 'Building...'"
        dependencies: ["test"]
    }
    "test": {
        description: "Run tests"
        command: "echo 'Testing...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let plan = executor
            .build_execution_plan(&["build".to_string()])
            .unwrap();

        // Should have 2 levels: [test], [build]
        assert_eq!(plan.levels.len(), 2);
        assert_eq!(plan.levels[0], vec!["test"]);
        assert_eq!(plan.levels[1], vec!["build"]);
    }

    #[tokio::test]
    async fn test_circular_dependency_detection() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "task1": {
        command: "echo 'Task 1'"
        dependencies: ["task2"]
    }
    "task2": {
        command: "echo 'Task 2'"
        dependencies: ["task1"]
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["task1".to_string()]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[tokio::test]
    async fn test_missing_task_error() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        command: "echo 'Building...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["nonexistent".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_missing_dependency_error() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "build": {
        command: "echo 'Building...'"
        dependencies: ["nonexistent"]
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let result = executor.build_execution_plan(&["build".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_complex_dependency_graph() {
        let tasks_cue = r#"package env

env: {}

tasks: {
    "deploy": {
        command: "echo 'Deploying...'"
        dependencies: ["build", "test"]
    }
    "build": {
        command: "echo 'Building...'"
        dependencies: ["compile"]
    }
    "test": {
        command: "echo 'Testing...'"
        dependencies: ["compile"]
    }
    "compile": {
        command: "echo 'Compiling...'"
    }
}"#;

        let (manager, temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let cache_config = crate::cache::CacheConfig {
            base_dir: temp_dir.path().join(".cache"),
            max_size: 1024 * 1024, // 1MB for tests
            mode: crate::cache::CacheMode::ReadWrite,
            inline_threshold: 4096,
            env_filter: Default::default(),
            task_env_filters: std::collections::HashMap::new(),
        };
        let executor =
            TaskExecutor::new_with_config(manager, temp_dir.path().to_path_buf(), cache_config)
                .await
                .unwrap();

        let plan = executor
            .build_execution_plan(&["deploy".to_string()])
            .unwrap();

        // Should have 3 levels: [compile], [build, test], [deploy]
        assert_eq!(plan.levels.len(), 3);
        assert_eq!(plan.levels[0], vec!["compile"]);
        assert_eq!(plan.levels[1].len(), 2);
        assert!(plan.levels[1].contains(&"build".to_string()));
        assert!(plan.levels[1].contains(&"test".to_string()));
        assert_eq!(plan.levels[2], vec!["deploy"]);
    }
}
