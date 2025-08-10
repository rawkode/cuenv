use crate::{MonorepoTaskRegistry, TaskBuilder};
use ::tracing::Instrument;
use cuenv_cache::config::CacheConfiguration;
use cuenv_cache::{concurrent::action::ActionCache, CacheManager};
use cuenv_config::{Config, TaskConfig};
use cuenv_core::{Error, Result};
use cuenv_core::{TaskDefinition, TaskExecutionMode};
use cuenv_env::manager::EnvManager;
use cuenv_utils::cleanup::handler::ProcessGuard;
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
    /// Built and validated task definitions
    pub tasks: HashMap<String, TaskDefinition>,
}

/// Main task executor that handles dependency resolution and execution
#[derive(Clone)]
pub struct TaskExecutor {
    /// Pre-loaded configuration from ConfigLoader
    config: Arc<Config>,
    env_manager: EnvManager,
    working_dir: PathBuf,
    cache_manager: Arc<CacheManager>,
    action_cache: Arc<ActionCache>,
    cache_config: CacheConfiguration,
    /// Task builder for Phase 3 architecture
    task_builder: TaskBuilder,
    /// Optional registry for cross-package task execution in monorepos
    monorepo_registry: Option<Arc<MonorepoTaskRegistry>>,
    /// Track executed tasks to avoid re-execution in cross-package scenarios
    executed_tasks: Arc<Mutex<HashSet<String>>>,
}

impl TaskExecutor {
    /// Create a new task executor with pre-loaded configuration
    ///
    /// This replaces the old constructor pattern and now requires a pre-loaded
    /// `Config` from the `ConfigLoader`. The `EnvManager` is created internally
    /// and has its configuration already applied.
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        // Create EnvManager with the pre-loaded configuration
        let mut env_manager = EnvManager::new(config.clone());
        env_manager.apply_config().await?;

        let working_dir = config.working_directory.clone();

        // Setup cache configuration from the Config
        let mut cache_config = CacheConfiguration::default();
        cache_config.global.enabled = config.runtime_settings.cache_enabled;

        let cache_config_struct = Self::create_cache_config_struct(&cache_config)?;
        let mut cache_manager = CacheManager::new(cache_config_struct).await?;

        // Apply task-specific cache environment configurations from Config
        let tasks_ref = config.filter_tasks_by_capabilities();
        let tasks: HashMap<String, TaskConfig> =
            tasks_ref.into_iter().map(|(k, v)| (k, v.clone())).collect();
        cache_manager.apply_task_configs(&tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with working directory from Config
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            config: config.clone(),
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config,
            task_builder,
            monorepo_registry: None,
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Create a new task executor with monorepo registry for cross-package execution
    pub async fn new_with_registry(
        config: Arc<Config>,
        registry: MonorepoTaskRegistry,
    ) -> Result<Self> {
        // Create EnvManager with the pre-loaded configuration
        let mut env_manager = EnvManager::new(config.clone());
        env_manager.apply_config().await?;

        let working_dir = config.working_directory.clone();

        // Setup cache configuration from the Config
        let mut cache_config = CacheConfiguration::default();
        cache_config.global.enabled = config.runtime_settings.cache_enabled;

        let cache_config_struct = Self::create_cache_config_struct(&cache_config)?;
        let cache_manager = CacheManager::new(cache_config_struct).await?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with working directory from Config
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            config: config.clone(),
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config,
            task_builder,
            monorepo_registry: Some(Arc::new(registry)),
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Create a new task executor with custom cache config (for testing)
    #[cfg(test)]
    pub async fn new_with_config(
        config: Arc<Config>,
        cache_config: cuenv_cache::CacheConfig,
    ) -> Result<Self> {
        // Create EnvManager with the pre-loaded configuration
        let mut env_manager = EnvManager::new(config.clone());
        env_manager.apply_config().await?;

        let working_dir = config.working_directory.clone();
        let cache_configuration = CacheConfiguration::default();
        let mut cache_manager = CacheManager::new(cache_config).await?;

        // Apply task-specific cache environment configurations from Config
        let tasks_ref = config.filter_tasks_by_capabilities();
        let tasks: HashMap<String, TaskConfig> =
            tasks_ref.into_iter().map(|(k, v)| (k, v.clone())).collect();
        cache_manager.apply_task_configs(&tasks)?;

        let cache_manager = Arc::new(cache_manager);
        let action_cache = cache_manager.action_cache();

        // Create TaskBuilder with working directory from Config
        let task_builder = TaskBuilder::new(working_dir.clone());

        Ok(Self {
            config: config.clone(),
            env_manager,
            working_dir,
            cache_manager,
            action_cache,
            cache_config: cache_configuration,
            task_builder,
            monorepo_registry: None,
            executed_tasks: Arc::new(Mutex::new(HashSet::new())),
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

    /// Execute a task
    pub async fn execute(&mut self, task_name: &str) -> Result<()> {
        let exit_code = self.execute_task(task_name, &[]).await?;

        if exit_code != 0 {
            return Err(Error::configuration(format!(
                "Task '{}' failed with exit code {}",
                task_name, exit_code
            )));
        }

        Ok(())
    }

    /// Get a topologically sorted list of tasks to execute
    pub fn get_execution_order(&self, task_name: &str) -> Result<Vec<String>> {
        let plan = self.build_execution_plan(&[task_name.to_string()])?;

        // Flatten the levels into a single list
        let mut order = Vec::new();
        for level in plan.levels {
            for task in level {
                order.push(task);
            }
        }

        Ok(order)
    }

    /// Check if a task has been executed (for testing)
    pub fn is_executed(&self, task_name: &str) -> bool {
        self.executed_tasks
            .lock()
            .map(|guard| guard.contains(task_name))
            .unwrap_or(false)
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
        // TODO: Add tracing when moved to workspace
        let _pipeline_span = tracing::info_span!("pipeline", tasks = plan.tasks.len());
        let pipeline_guard = _pipeline_span.enter();

        tracing::info!(
            requested_tasks = ?task_names,
            total_tasks = %plan.tasks.len(),
            levels = %plan.levels.len(),
            "Starting task execution pipeline"
        );

        // Execute tasks level by level
        for (level_idx, level) in plan.levels.iter().enumerate() {
            // TODO: Add tracing when moved to workspace
            let _level_span = tracing::info_span!("level", idx = level_idx, tasks = level.len());
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
                let task_definition = match plan.tasks.get(task_name) {
                    Some(definition) => definition.clone(),
                    None => {
                        return Err(Error::configuration(format!(
                            "Task '{task_name}' not found in execution plan"
                        )));
                    }
                };

                // Determine working directory based on whether this is a cross-package task
                let working_dir = if let Some(ref registry) = self.monorepo_registry {
                    // For cross-package tasks, get the package path from the registry
                    if let Some(task) = registry.get_task(task_name) {
                        task.package_path.clone()
                    } else {
                        self.working_dir.clone()
                    }
                } else {
                    self.working_dir.clone()
                };
                let task_args = args.to_vec();
                let failed_tasks = Arc::clone(&failed_tasks);
                let task_name_owned = task_name.clone();
                let action_cache = Arc::clone(&self.action_cache);
                let env_manager_clone = self.env_manager.clone();
                let cache_config = self.cache_config.clone();
                let executed_tasks = Arc::clone(&self.executed_tasks);

                // Create task span
                // TODO: Add tracing when moved to workspace
                let task_span = tracing::info_span!("task", name = task_name_owned.as_str());

                join_set.spawn(
                    async move {
                        let start_time = Instant::now();

                        // Publish task started event
                        {
                            let event_bus = cuenv_core::events::global_event_bus();
                            let _ = event_bus
                                .publish(cuenv_core::SystemEvent::Task(
                                    cuenv_core::TaskEvent::TaskStarted {
                                        task_name: task_name_owned.clone(),
                                        task_id: task_name_owned.clone(),
                                    },
                                ))
                                .await;
                        }

                        // Disabled: Detailed task configuration events (not essential for now)
                        if false {
                            // event_bus
                            //     .publish(TaskEvent::Started {
                            //         task_name: task_name_owned.clone(),
                            //         timestamp: start_time,
                            //     })
                            //     .await;

                            // Send task configuration info as progress messages
                            // Show capabilities for this task's command
                            if let TaskExecutionMode::Command { command } =
                                &task_definition.execution_mode
                            {
                                let capabilities =
                                    env_manager_clone.get_command_capabilities(command);
                                if !capabilities.is_empty() {
                                    // event_bus
                                    //     .publish(TaskEvent::Progress {
                                    //         task_name: task_name_owned.clone(),
                                    //         message: format!(
                                    //             "Capabilities: {}",
                                    //             capabilities.join(", ")
                                    //         ),
                                    //     })
                                    //     .await;
                                }
                            }

                            if !task_definition.shell.is_empty() {
                                // event_bus
                                //     .publish(TaskEvent::Progress {
                                //         task_name: task_name_owned.clone(),
                                //         message: format!("Shell: {}", shell),
                                //     })
                                //     .await;
                            }

                            if task_definition.timeout.as_millis() > 0 {
                                // event_bus
                                //     .publish(TaskEvent::Progress {
                                //         task_name: task_name_owned.clone(),
                                //         message: format!("Timeout: {}s", timeout),
                                //     })
                                //     .await;
                            }

                            // TODO: Fix when TaskCacheConfig is properly exposed
                            if false {
                                // event_bus
                                //     .publish(cuenv_tui::TaskEvent::Progress {
                                //         task_name: task_name_owned.clone(),
                                //         message: "Cache: enabled".to_string(),
                                //     })
                                //     .await;
                            }

                            if !task_definition.working_directory.as_os_str().is_empty() {
                                // event_bus
                                //     .publish(TaskEvent::Progress {
                                //         task_name: task_name_owned.clone(),
                                //         message: format!("Working dir: {}", working_dir),
                                //     })
                                //     .await;
                            }

                            if let Some(security) = &task_definition.security {
                                let mut restrictions = Vec::new();
                                if security.restrict_disk {
                                    restrictions.push("disk");
                                }
                                if security.restrict_network {
                                    restrictions.push("network");
                                }
                                if !restrictions.is_empty() {
                                    // event_bus
                                    //     .publish(TaskEvent::Progress {
                                    //         task_name: task_name_owned.clone(),
                                    //         message: format!(
                                    //             "Security: {} restricted",
                                    //             restrictions.join(", ")
                                    //         ),
                                    //     })
                                    //     .await;
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
                            &task_definition,
                            &task_args,
                        )
                        .await
                        {
                            Ok(status) => {
                                let _duration_ms = start_time.elapsed().as_millis() as u64;
                                if status != 0 {
                                    if let Ok(mut guard) = failed_tasks.lock() {
                                        guard.push((task_name_owned.clone(), status));
                                    } else {
                                        tracing::error!(
                                            "Failed to acquire lock for failed tasks tracking"
                                        );
                                    }

                                    // Publish task failed event
                                    {
                                        let event_bus = cuenv_core::events::global_event_bus();
                                        let _duration_ms = start_time.elapsed().as_millis() as u64;
                                        let _ = event_bus
                                            .publish(cuenv_core::SystemEvent::Task(
                                                cuenv_core::TaskEvent::TaskFailed {
                                                    task_name: task_name_owned.clone(),
                                                    task_id: task_name_owned.clone(),
                                                    error: format!(
                                                        "Task exited with code {}",
                                                        status
                                                    ),
                                                },
                                            ))
                                            .await;
                                    }

                                    // TODO: Add tracing when moved to workspace
                                    // task_completed(&task_name_owned, duration_ms, false);
                                } else {
                                    // Mark task as executed
                                    if let Ok(mut guard) = executed_tasks.lock() {
                                        guard.insert(task_name_owned.clone());
                                    }

                                    // Publish task completed event
                                    {
                                        let event_bus = cuenv_core::events::global_event_bus();
                                        let _duration_ms = start_time.elapsed().as_millis() as u64;
                                        let _ = event_bus
                                            .publish(cuenv_core::SystemEvent::Task(
                                                cuenv_core::TaskEvent::TaskCompleted {
                                                    task_name: task_name_owned.clone(),
                                                    task_id: task_name_owned.clone(),
                                                    duration_ms: _duration_ms,
                                                },
                                            ))
                                            .await;
                                    }

                                    // TODO: Add tracing when moved to workspace
                                    tracing::info!(
                                        task = task_name_owned.as_str(),
                                        duration_ms = _duration_ms,
                                        "Task completed"
                                    );
                                }
                                status
                            }
                            Err(e) => {
                                let _duration_ms = start_time.elapsed().as_millis() as u64;
                                if let Ok(mut guard) = failed_tasks.lock() {
                                    guard.push((task_name_owned.clone(), -1));
                                } else {
                                    tracing::error!(
                                        "Failed to acquire lock for failed tasks tracking"
                                    );
                                }

                                // Publish task failed event
                                {
                                    let event_bus = cuenv_core::events::global_event_bus();
                                    let _duration_ms = start_time.elapsed().as_millis() as u64;
                                    let _ = event_bus
                                        .publish(cuenv_core::SystemEvent::Task(
                                            cuenv_core::TaskEvent::TaskFailed {
                                                task_name: task_name_owned.clone(),
                                                task_id: task_name_owned.clone(),
                                                error: e.to_string(),
                                            },
                                        ))
                                        .await;
                                }

                                tracing::error!(
                                    task_name = %task_name_owned,
                                    error = %e,
                                    "Task execution failed"
                                );
                                // TODO: Add tracing when moved to workspace
                                // task_completed(&task_name_owned, duration_ms, false);
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
        // If we have a monorepo registry, use it for cross-package task resolution
        if let Some(ref registry) = self.monorepo_registry {
            return self.build_monorepo_execution_plan(task_names, registry);
        }

        let all_task_configs = self.env_manager.get_tasks();

        // Validate that all requested tasks exist
        for task_name in task_names {
            if !all_task_configs.contains_key(task_name) {
                return Err(Error::configuration(format!(
                    "Task '{task_name}' not found"
                )));
            }
        }

        // Build task definitions using TaskBuilder
        let task_definitions = self.task_builder.build_tasks(all_task_configs.clone())?;

        // Build dependency graph using task definitions
        let mut task_dependencies = HashMap::with_capacity(task_definitions.len());
        let mut visited = HashSet::with_capacity(task_definitions.len());
        let mut stack = HashSet::new();

        for task_name in task_names {
            Self::collect_dependencies_from_definitions(
                task_name,
                &task_definitions,
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
            if let Some(definition) = task_definitions.get(task_name) {
                plan_tasks.insert(task_name.clone(), definition.clone());
            }
        }

        Ok(TaskExecutionPlan {
            levels,
            tasks: plan_tasks,
        })
    }

    /// Build an execution plan for monorepo with cross-package tasks
    fn build_monorepo_execution_plan(
        &self,
        task_names: &[String],
        registry: &MonorepoTaskRegistry,
    ) -> Result<TaskExecutionPlan> {
        let mut all_tasks = HashMap::new();
        let mut task_dependencies = HashMap::new();
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        // Validate and collect tasks from registry
        for task_name in task_names {
            let _task = registry
                .get_task(task_name)
                .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

            Self::collect_monorepo_dependencies(
                task_name,
                registry,
                &mut all_tasks,
                &mut task_dependencies,
                &mut visited,
                &mut stack,
            )?;
        }

        // Build task definitions using TaskBuilder
        let task_definitions = self.task_builder.build_tasks(all_tasks)?;

        // Topological sort to determine execution order
        let levels = self.topological_sort(&task_dependencies)?;

        Ok(TaskExecutionPlan {
            levels,
            tasks: task_definitions,
        })
    }

    /// Recursively collect dependencies for monorepo tasks
    fn collect_monorepo_dependencies(
        task_name: &str,
        registry: &MonorepoTaskRegistry,
        all_tasks: &mut HashMap<String, TaskConfig>,
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

        let task = registry
            .get_task(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

        // Add task config to all_tasks
        all_tasks.insert(task_name.to_owned(), task.config.clone());

        let mut dependencies = Vec::new();

        // Process dependencies, resolving cross-package references
        if let Some(ref deps) = task.config.dependencies {
            for dep in deps {
                // Check if this is a cross-package reference
                let full_dep_name = if dep.contains(':') {
                    // Already a full cross-package reference
                    dep.clone()
                } else {
                    // Local task reference, add package prefix
                    format!("{}:{}", task.package_name, dep)
                };

                // Validate dependency exists
                if registry.get_task(&full_dep_name).is_none() {
                    return Err(Error::configuration(format!(
                        "Dependency '{full_dep_name}' of task '{task_name}' not found"
                    )));
                }

                dependencies.push(full_dep_name.clone());

                // Recursively collect dependencies
                Self::collect_monorepo_dependencies(
                    &full_dep_name,
                    registry,
                    all_tasks,
                    task_dependencies,
                    visited,
                    stack,
                )?;
            }
        }

        task_dependencies.insert(task_name.to_owned(), dependencies);
        visited.insert(task_name.to_owned());
        stack.remove(task_name);

        Ok(())
    }

    /// Recursively collect all dependencies for a task
    #[allow(dead_code)]
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

        let task_definition = all_tasks
            .get(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

        let dependencies = task_definition.dependencies.clone().unwrap_or_default();

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

    /// Recursively collect task dependencies from task definitions (Phase 3)
    fn collect_dependencies_from_definitions(
        task_name: &str,
        all_tasks: &HashMap<String, TaskDefinition>,
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

        let task_definition = all_tasks
            .get(task_name)
            .ok_or_else(|| Error::configuration(format!("Task '{task_name}' not found")))?;

        let dependencies = task_definition.dependency_names();

        // Validate and collect dependencies
        for dep_name in &dependencies {
            if !all_tasks.contains_key(dep_name) {
                return Err(Error::configuration(format!(
                    "Dependency '{dep_name}' of task '{task_name}' not found"
                )));
            }

            Self::collect_dependencies_from_definitions(
                dep_name,
                all_tasks,
                task_dependencies,
                visited,
                stack,
            )?;
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
    ) -> Result<cuenv_cache::config::CacheConfig> {
        // TODO: Fix when CacheConfig is properly exposed
        let mut config = cuenv_cache::config::CacheConfig::default();

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
            return Self::execute_single_task(
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

                let exit_code = Self::execute_single_task(
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

    /// Execute a single task
    async fn execute_single_task(
        task_name: &str,
        task_definition: &TaskDefinition,
        _working_dir: &Path,
        args: &[String],
        audit_mode: bool,
        capture_output: bool,
    ) -> Result<i32> {
        // Determine what to execute from TaskDefinition
        let (shell, script_content) = match &task_definition.execution_mode {
            TaskExecutionMode::Command { command } => {
                // Add user args to the command
                let full_command = if args.is_empty() {
                    command.clone()
                } else {
                    format!("{} {}", command, args.join(" "))
                };
                (task_definition.shell.clone(), full_command)
            }
            TaskExecutionMode::Script { content } => {
                (task_definition.shell.clone(), content.clone())
            }
        };

        // Validate shell command for security
        // Use a static set for allowed shells to avoid repeated allocations
        static ALLOWED_SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "pwsh", "powershell"];
        let allowed_shells: HashSet<String> =
            ALLOWED_SHELLS.iter().map(|&s| s.to_string()).collect();

        cuenv_security::SecurityValidator::validate_command(&shell, &allowed_shells)?;

        // Validate script content for dangerous patterns
        cuenv_security::SecurityValidator::validate_shell_expansion(&script_content)?;

        // Validate user arguments
        if !args.is_empty() {
            cuenv_security::SecurityValidator::validate_command_args(args)?;
        }

        // Use the working directory from task definition
        let exec_dir = task_definition.working_directory.clone();

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
                    // TODO: Add apply_default_limits when moved to workspace
                    match Ok::<(), std::io::Error>(()) {
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
        if let Some(security) = &task_definition.security {
            use cuenv_security::AccessRestrictions;
            let mut restrictions =
                AccessRestrictions::new(security.restrict_disk, security.restrict_network);

            // Add allowed paths
            for path in &security.read_only_paths {
                restrictions.add_read_only_path(path);
            }
            for path in &security.write_only_paths {
                restrictions.add_read_write_path(path);
            }

            if audit_mode {
                restrictions.enable_audit_mode();
                // TODO: Add tracing when moved to workspace
                // task_progress(task_name, None, "Running task in audit mode...");

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

            let _task_name_stdout = task_name.to_string();
            let _task_name_stderr = task_name.to_string();

            // Spawn thread to read stdout
            let stdout_handle = stdout.map(|stdout| {
                std::thread::spawn(move || {
                    // Create a tokio runtime for this thread
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();

                    if let Ok(_rt) = rt {
                        let reader = BufReader::new(stdout);
                        for _line in reader.lines().map_while(|result| result.ok()) {
                            // Try to publish to event bus
                            // TODO: Add event publishing through proper abstraction
                            if false {
                                // let task_name = task_name_stdout.clone();
                                // let event = TaskEvent::Log {
                                //     task_name,
                                //     stream: LogStream::Stdout,
                                //     content: line,
                                // };
                                //
                                // // Use spawn instead of block_on to avoid blocking the runtime
                                // let event_bus_clone = event_bus.clone();
                                // rt.spawn(async move {
                                //     event_bus_clone.publish(event).await;
                                // });
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

                    if let Ok(_rt) = rt {
                        let reader = BufReader::new(stderr);
                        for _line in reader.lines().map_while(|result| result.ok()) {
                            // Try to publish to event bus
                            // TODO: Add event publishing through proper abstraction
                            if false {
                                // let task_name = task_name_stderr.clone();
                                // let event = TaskEvent::Log {
                                //     task_name,
                                //     stream: LogStream::Stderr,
                                //     content: line,
                                // };
                                //
                                // // Use spawn instead of block_on to avoid blocking the runtime
                                // let event_bus_clone = event_bus.clone();
                                // rt.spawn(async move {
                                //     event_bus_clone.publish(event).await;
                                // });
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
        let timeout = task_definition.timeout;

        let mut guard = ProcessGuard::new(child, timeout);

        // Wait for completion with timeout (use async version to avoid blocking the runtime)
        let status = guard.wait_with_timeout_async().await.map_err(|e| {
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
    pub fn get_env_vars(&self) -> HashMap<String, String> {
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
    pub fn get_cache_statistics(&self) -> Result<cuenv_cache::manager::CacheStatistics> {
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

    async fn create_test_env_manager_with_tasks(tasks_cue: &str) -> (Arc<Config>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join("env.cue");
        fs::write(&env_file, tasks_cue).unwrap();

        use cuenv_config::{ConfigLoader, PackageInfo, RuntimeSettings, SecurityContext};
        let mut loader = ConfigLoader::new();
        let config = loader
            .environment("dev".to_string())
            .capabilities(vec![])
            .working_directory(temp_dir.path().to_path_buf())
            .original_environment(std::env::vars().collect())
            .runtime_settings(RuntimeSettings {
                cache_enabled: true,
                ..Default::default()
            })
            .security_context(SecurityContext::default())
            .package_info(None)
            .load()
            .await
            .unwrap();
        (config, temp_dir)
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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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

        let (config, _temp_dir) = create_test_env_manager_with_tasks(tasks_cue).await;
        let executor = TaskExecutor::new(config).await.unwrap();

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
