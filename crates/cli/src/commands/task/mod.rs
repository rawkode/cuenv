mod display;
mod formatter;

use clap::Subcommand;
use cuenv_config::{Config, TaskGroupMode, TaskNode};
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR};
use cuenv_env::manager::environment::SupervisorMode;
use cuenv_env::EnvManager;
use cuenv_task::TaskExecutor;
use std::env;
use std::sync::Arc;

use self::display::{display_group_contents, display_task_tree};

/// Execute the simplified task command
#[allow(clippy::too_many_arguments)]
pub async fn execute_task_command(
    config: Arc<Config>,
    task_or_group: Option<String>,
    args: Vec<String>,
    environment: Option<String>,
    capabilities: Vec<String>,
    audit: bool,
    verbose: bool,
    output_format: String,
    trace_output: bool,
    graph: bool,
) -> Result<()> {
    // If --graph flag is set, show the dependency graph instead of executing
    if graph {
        return display_dependency_graph(config, task_or_group).await;
    }

    match task_or_group {
        None => {
            // No arguments: list all tasks
            list_tasks(config, verbose, None).await
        }
        Some(name) => {
            // Check if it's a task or a group
            let tasks = config.get_tasks();

            // First check if it's a direct task
            if tasks.contains_key(&name) {
                // It's a task - run it
                execute_task(
                    config,
                    environment,
                    capabilities,
                    name,
                    args,
                    audit,
                    output_format.clone(),
                    trace_output,
                )
                .await
            } else if args.is_empty() {
                // No additional args - check if it's a group
                let prefix = format!("{name}.");
                let has_subtasks = tasks.keys().any(|k| k.starts_with(&prefix));

                if has_subtasks {
                    // It's a group - check its mode to decide whether to execute or list
                    let task_nodes = config.get_task_nodes();
                    if let Some(TaskNode::Group { mode, .. }) = task_nodes.get(&name) {
                        match mode {
                            TaskGroupMode::Group => {
                                // Group mode: just list tasks
                                list_tasks(config, verbose, Some(name)).await
                            }
                            TaskGroupMode::Parallel
                            | TaskGroupMode::Sequential
                            | TaskGroupMode::Workflow => {
                                // Executable modes: run all tasks in the group
                                execute_task_group(
                                    config.clone(),
                                    environment,
                                    capabilities,
                                    name,
                                    audit,
                                    output_format,
                                    trace_output,
                                )
                                .await
                            }
                        }
                    } else {
                        // Fallback to listing if we can't determine the mode
                        list_tasks(config, verbose, Some(name)).await
                    }
                } else {
                    // Not found as task or group
                    eprintln!("Task or group '{name}' not found");
                    eprintln!("Run 'cuenv task' to see available tasks");
                    std::process::exit(1)
                }
            } else {
                // Has additional args - try as group + subtask
                let subtask_name = format!("{}.{}", name, args[0]);
                if tasks.contains_key(&subtask_name) {
                    // It's a subtask - run it with remaining args
                    let mut remaining_args = args;
                    remaining_args.remove(0);
                    execute_task(
                        config,
                        environment,
                        capabilities,
                        subtask_name,
                        remaining_args,
                        audit,
                        output_format.clone(),
                        trace_output,
                    )
                    .await
                } else {
                    // Try running the original name as a task with all args
                    if tasks.contains_key(&name) {
                        execute_task(
                            config,
                            environment,
                            capabilities,
                            name,
                            args,
                            audit,
                            output_format,
                            trace_output,
                        )
                        .await
                    } else {
                        eprintln!("Task '{name}' not found");
                        eprintln!("Run 'cuenv task' to see available tasks");
                        std::process::exit(1)
                    }
                }
            }
        }
    }
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List available tasks
    #[command(visible_alias = "l")]
    List {
        /// Show task descriptions
        #[arg(short, long)]
        verbose: bool,

        /// Optional group name to list tasks for
        group: Option<String>,
    },

    /// Run a task with the loaded environment
    #[command(visible_alias = "r")]
    Run {
        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,

        /// Task name to execute
        task_name: String,

        /// Arguments to pass to the task (after --)
        #[arg(last = true)]
        task_args: Vec<String>,

        /// Run in audit mode to see file and network access without restrictions
        #[arg(long)]
        audit: bool,

        /// Output format for task execution (tui, simple, or spinner)
        #[arg(long, value_name = "FORMAT", default_value = "spinner")]
        output: String,

        /// Generate Chrome trace output file
        #[arg(long)]
        trace_output: bool,
    },
}

async fn list_tasks(
    config: std::sync::Arc<cuenv_config::Config>,
    verbose: bool,
    group_filter: Option<String>,
) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;

    // Get task nodes to display with execution modes
    let task_nodes = config.get_task_nodes();

    if task_nodes.is_empty() {
        println!("No tasks defined in the CUE package");
        return Ok(());
    }

    // Check if terminal supports colors
    let use_color = atty::is(atty::Stream::Stdout);

    // If a group filter is specified, show that specific group
    if let Some(ref group) = group_filter {
        if let Some(node) = task_nodes.get(group) {
            match node {
                TaskNode::Group {
                    description,
                    mode,
                    tasks,
                } => {
                    display_group_contents(
                        group,
                        description.as_deref(),
                        mode,
                        tasks,
                        verbose,
                        use_color,
                    );
                }
                TaskNode::Task(config) => {
                    println!("'{group}' is a single task, not a group");
                    if let Some(desc) = &config.description {
                        println!("  Description: {desc}");
                    }
                }
            }
        } else {
            println!("No task or group named '{group}' found");
            println!("Run 'cuenv task' to see all available tasks");
        }
        return Ok(());
    }

    // Display all tasks in tree format
    display_task_tree(task_nodes, verbose, use_color);
    Ok(())
}

// Display functions moved to display module

#[allow(clippy::too_many_arguments)]
async fn execute_task(
    _config: std::sync::Arc<cuenv_config::Config>,
    environment: Option<String>,
    capabilities: Vec<String>,
    task_name: String,
    task_args: Vec<String>,
    audit: bool,
    output_format: String,
    trace_output: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;
    let mut env_manager = EnvManager::new();

    let env_name = environment.or_else(|| env::var(CUENV_ENV_VAR).ok());
    let mut caps = capabilities;
    if caps.is_empty() {
        if let Ok(env_caps) = env::var(CUENV_CAPABILITIES_VAR) {
            caps = env_caps
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    env_manager
        .load_env_with_options(
            &current_dir,
            env_name,
            caps,
            None,
            SupervisorMode::Foreground,
        )
        .await?;

    // Check if this might be a group/subtask pattern (e.g., "fmt" with first arg "check")
    // First try the task as-is, then try as group.subtask if not found
    let actual_task_name;
    let mut actual_args = task_args.clone();

    if let Some(_task) = env_manager.get_task(&task_name) {
        // Task exists as-is
        actual_task_name = task_name.clone();
    } else if !task_args.is_empty() {
        // Task doesn't exist, try treating it as a group with the first arg as subtask
        let potential_subtask = format!("{}.{}", task_name, task_args[0]);
        if env_manager.get_task(&potential_subtask).is_some() {
            // It's a subtask! Remove the first arg since it's part of the task name
            actual_task_name = potential_subtask;
            actual_args.remove(0);
        } else {
            // Neither the task nor the group.subtask exists
            actual_task_name = task_name.clone();
        }
    } else {
        // No args and task doesn't exist
        actual_task_name = task_name.clone();
    };

    // Check if this is a cross-package task reference OR a local task with cross-package dependencies
    let has_cross_package_deps = if let Some(task) = env_manager.get_task(&actual_task_name) {
        task.dependencies
            .as_ref()
            .map(|deps| deps.iter().any(|d| d.contains(':')))
            .unwrap_or(false)
    } else {
        false
    };

    if (actual_task_name.contains(':') || has_cross_package_deps)
        && crate::monorepo::is_monorepo(&current_dir)
    {
        // Handle cross-package task execution
        let status = crate::monorepo::execute_monorepo_task(
            &current_dir,
            &actual_task_name,
            &actual_args,
            audit,
        )
        .await?;
        std::process::exit(status);
    } else if env_manager.get_task(&actual_task_name).is_some() {
        // Execute the specified task
        let executor = TaskExecutor::new(env_manager, current_dir).await?;
        // Use the formatter module to execute with the appropriate output format
        let status = formatter::execute_with_formatter(
            &executor,
            &actual_task_name,
            &actual_args,
            audit,
            &output_format,
            trace_output,
        )
        .await?;
        std::process::exit(status);
    } else {
        // Check if this might be a task group
        let prefix = format!("{task_name}.");
        let all_tasks = env_manager.list_tasks();
        let group_tasks: Vec<_> = all_tasks
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .collect();

        if !group_tasks.is_empty() {
            eprintln!("'{task_name}' is a task group. Available tasks:");
            for (name, _) in group_tasks {
                let task_name = &name[prefix.len()..];
                eprintln!("  {task_name}");
            }
            eprintln!();
            eprintln!("Run 'cuenv task {task_name} <task>' to execute a task");
        } else {
            eprintln!("Task '{task_name}' not found");
            eprintln!("Run 'cuenv task list' to see available tasks");
        }
        std::process::exit(1);
    }
}

async fn execute_task_group(
    config: std::sync::Arc<cuenv_config::Config>,
    environment: Option<String>,
    capabilities: Vec<String>,
    group_name: String,
    audit: bool,
    output_format: String,
    trace_output: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;
    let mut env_manager = EnvManager::new();

    let env_name = environment.or_else(|| env::var(CUENV_ENV_VAR).ok());
    let mut caps = capabilities;

    // Add capabilities from environment variable if set
    if let Ok(env_caps) = env::var(CUENV_CAPABILITIES_VAR) {
        caps.extend(env_caps.split(',').map(|s| s.trim().to_string()));
    }

    // Load the environment with applied environment and capabilities
    env_manager
        .load_env_with_options(
            &current_dir,
            env_name,
            caps,
            None,
            SupervisorMode::Foreground,
        )
        .await?;

    // Get the group's execution mode for display
    let task_nodes = config.get_task_nodes();
    let mode = if let Some(TaskNode::Group { mode, .. }) = task_nodes.get(&group_name) {
        mode.clone()
    } else {
        return Err(cuenv_core::Error::configuration(format!(
            "Group '{group_name}' not found"
        )));
    };

    // Handle Group mode (organization only)
    if matches!(mode, TaskGroupMode::Group) {
        eprintln!("Group '{group_name}' is for organization only and cannot be executed");
        eprintln!("Run 'cuenv task {group_name}' to see available tasks");
        std::process::exit(1);
    }

    println!(
        "Executing group '{}' in {} mode",
        group_name,
        match &mode {
            TaskGroupMode::Workflow => "workflow",
            TaskGroupMode::Sequential => "sequential",
            TaskGroupMode::Parallel => "parallel",
            TaskGroupMode::Group => "group", // Should not reach here
        }
    );

    // Create executor and use unified DAG for all execution modes
    let executor = TaskExecutor::new(env_manager, current_dir).await?;

    // Use unified DAG execution - this handles all modes (Sequential, Parallel, Workflow) properly
    let status = formatter::execute_with_formatter(
        &executor,
        &group_name, // Pass the group name directly to unified DAG
        &[],
        audit,
        &output_format,
        trace_output,
    )
    .await?;

    if status != 0 {
        std::process::exit(status);
    }

    Ok(())
}

/// Display the dependency graph for tasks
async fn display_dependency_graph(config: Arc<Config>, task_or_group: Option<String>) -> Result<()> {
    use cuenv_env::EnvManager;
    use cuenv_task::TaskExecutor;
    use cuenv_env::manager::environment::SupervisorMode;
    
    let current_dir = std::env::current_dir().unwrap();
    
    // Create environment manager and load environment
    let mut env_manager = EnvManager::new();
    
    env_manager
        .load_env_with_options(
            &current_dir,
            None, // Use default environment
            vec![], // No additional capabilities
            None,
            SupervisorMode::Foreground,
        )
        .await?;

    // Create executor to build the unified DAG
    let executor = TaskExecutor::new(env_manager, current_dir.clone()).await?;

    match task_or_group {
        Some(name) => {
            // Build DAG for specific task or group
            let dag = executor.build_unified_dag(&[name.clone()])?;
            
            println!("Dependency Graph for: {}", name);
            println!("{}", "=".repeat(50));
            display_dag_visualization(&dag, &name);
        }
        None => {
            // Show all available tasks and their basic info
            println!("All Available Tasks and Groups:");
            println!("{}", "=".repeat(50));
            
            let tasks = config.get_tasks();
            let task_nodes = config.get_task_nodes();
            
            for (name, _) in tasks.iter() {
                // Skip subtasks (they contain dots)
                if name.contains('.') {
                    continue;
                }
                
                // Build individual DAG to show dependencies
                if let Ok(dag) = executor.build_unified_dag(&[name.clone()]) {
                    display_dag_visualization(&dag, name);
                    println!(); // Add spacing between tasks
                }
            }
            
            // Also show groups
            for (name, node) in task_nodes.iter() {
                if let TaskNode::Group { mode, tasks, .. } = node {
                    if !tasks.is_empty() {
                        println!("📁 Group: {} ({})", name, match mode {
                            TaskGroupMode::Sequential => "sequential",
                            TaskGroupMode::Parallel => "parallel", 
                            TaskGroupMode::Workflow => "workflow",
                            TaskGroupMode::Group => "group",
                        });
                        
                        for (task_name, _) in tasks.iter() {
                            println!("  └─ {}", task_name);
                        }
                        println!();
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Display a visual representation of the DAG
fn display_dag_visualization(dag: &cuenv_task::UnifiedTaskDAG, root_name: &str) {
    println!("🎯 Task: {}", root_name);
    
    // Get the execution levels (topologically sorted)
    match dag.get_execution_levels() {
        Ok(levels) => {
            if levels.is_empty() {
                println!("  └─ No dependencies");
                return;
            }
            
            println!("  Execution Order:");
            for (level_num, level_tasks) in levels.iter().enumerate() {
                let level_prefix = format!("  Level {}:", level_num + 1);
                println!("{}", level_prefix);
                
                for (i, task) in level_tasks.iter().enumerate() {
                    let is_last = i == level_tasks.len() - 1;
                    let symbol = if is_last { "└─" } else { "├─" };
                    println!("    {} {}", symbol, task);
                    
                    // Show dependencies for this task
                    if let Some(deps) = dag.get_task_dependencies(task) {
                        if !deps.is_empty() {
                            let dep_prefix = if is_last { "    " } else { "    │" };
                            println!("{}   depends on: {}", dep_prefix, deps.join(", "));
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("  └─ Error building execution graph: {}", e);
        }
    }
    
    // Also show flattened task details
    let flattened = dag.get_flattened_tasks();
    if !flattened.is_empty() {
        println!();
        println!("  Flattened Execution Graph:");
        for task in flattened {
            let deps_str = if task.dependencies.is_empty() {
                "no dependencies".to_string()
            } else {
                format!("depends on: {}", task.dependencies.join(", "))
            };
            println!("    • {} ({})", task.id, deps_str);
        }
    }
}
