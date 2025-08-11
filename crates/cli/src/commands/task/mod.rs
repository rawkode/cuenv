use clap::Subcommand;
use cuenv_config::{Config, TaskConfig};
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR};
use cuenv_env::EnvManager;
use cuenv_task::TaskExecutor;
use std::env;
use std::sync::Arc;

/// Execute the simplified task command
pub async fn execute_task_command(
    config: Arc<Config>,
    task_or_group: Option<String>,
    args: Vec<String>,
    environment: Option<String>,
    capabilities: Vec<String>,
    audit: bool,
    verbose: bool,
) -> Result<()> {
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
                execute_task(config, environment, capabilities, name, args, audit).await
            } else if args.is_empty() {
                // No additional args - check if it's a group
                let prefix = format!("{name}.");
                let has_subtasks = tasks.keys().any(|k| k.starts_with(&prefix));

                if has_subtasks {
                    // It's a group - list its tasks
                    list_tasks(config, verbose, Some(name)).await
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
                    )
                    .await
                } else {
                    // Try running the original name as a task with all args
                    if tasks.contains_key(&name) {
                        execute_task(config, environment, capabilities, name, args, audit).await
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
        #[arg(long, value_name = "FORMAT", default_value = "tui")]
        output: String,

        /// Generate Chrome trace output file
        #[arg(long)]
        trace_output: bool,
    },

    /// Execute a command directly with the loaded environment
    #[command(visible_alias = "e")]
    Exec {
        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,

        /// Command to run
        command: String,

        /// Arguments to pass to the command
        args: Vec<String>,

        /// Run in audit mode to see file and network access without restrictions
        #[arg(long)]
        audit: bool,
    },
}

impl TaskCommands {
    pub async fn execute(self, config: std::sync::Arc<cuenv_config::Config>) -> Result<()> {
        match self {
            TaskCommands::List { verbose, group } => list_tasks(config, verbose, group).await,
            TaskCommands::Run {
                environment,
                capabilities,
                task_name,
                task_args,
                audit,
                output: _,
                trace_output: _,
            } => {
                execute_task(
                    config.clone(),
                    environment,
                    capabilities,
                    task_name,
                    task_args,
                    audit,
                )
                .await
            }
            TaskCommands::Exec {
                environment,
                capabilities,
                command,
                args,
                audit,
            } => execute_command(config, environment, capabilities, command, args, audit).await,
        }
    }
}

async fn list_tasks(
    config: std::sync::Arc<cuenv_config::Config>,
    verbose: bool,
    group_filter: Option<String>,
) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;

    // Always use config to get task definitions for the current directory
    // Monorepo discovery is only for cross-package task execution, not listing
    let tasks = config.get_tasks();

    if tasks.is_empty() {
        println!("No tasks defined in the CUE package");
        return Ok(());
    }

    // If a group filter is specified, only show tasks in that group
    if let Some(ref group) = group_filter {
        let prefix = format!("{group}.");
        let mut group_tasks = Vec::new();

        for (name, task) in tasks.iter() {
            if name.starts_with(&prefix) {
                let task_name = &name[prefix.len()..];
                group_tasks.push((task_name, task));
            }
        }

        if group_tasks.is_empty() {
            println!("No tasks found in group '{group}'");
            println!("Run 'cuenv task list' to see all available tasks");
        } else {
            println!("Tasks in '{group}' group:");
            for (task_name, task) in group_tasks {
                if verbose {
                    match &task.description {
                        Some(desc) => println!("  {task_name}: {desc}"),
                        None => println!("  {task_name}"),
                    }
                } else {
                    println!("  {task_name}");
                }
            }
            println!();
            println!("Run 'cuenv task {group} <task>' to execute a task");
        }
        return Ok(());
    }

    println!("Available tasks:");

    // Build a hierarchical structure from the flattened tasks
    let mut root_tasks = std::collections::BTreeMap::new();
    let mut grouped_tasks = std::collections::BTreeMap::new();

    for (name, task) in tasks.iter() {
        if name.contains('.') {
            // This is a nested task (using dots now)
            grouped_tasks.insert(name.clone(), task);
        } else {
            // This is a root-level task
            root_tasks.insert(name.clone(), task);
        }
    }

    // Display all tasks, including groups without root tasks
    let mut displayed_groups = std::collections::HashSet::new();

    // First display root tasks and their subtasks
    for (name, task) in &root_tasks {
        if verbose {
            match &task.description {
                Some(desc) => println!("  {name}: {desc}"),
                None => println!("  {name}"),
            }
        } else {
            println!("  {name}");
        }

        // Display any subtasks for this root task
        let prefix = format!("{name}.");
        for (sub_name, sub_task) in &grouped_tasks {
            if sub_name.starts_with(&prefix) {
                displayed_groups.insert(sub_name.clone());
                // Extract the relative name (without the parent prefix)
                let relative_name = &sub_name[prefix.len()..];
                let indent_level = relative_name.matches('.').count() + 1;
                let indent = "  ".repeat(indent_level + 1);

                if verbose {
                    match &sub_task.description {
                        Some(desc) => println!("{indent}{relative_name}: {desc}"),
                        None => println!("{indent}{relative_name}"),
                    }
                } else {
                    println!("{indent}{relative_name}");
                }
            }
        }
    }

    // Now display grouped tasks that don't have a root task
    // Group them by their prefix
    let mut task_groups: std::collections::BTreeMap<String, Vec<(String, &TaskConfig)>> =
        std::collections::BTreeMap::new();

    for (name, task) in &grouped_tasks {
        if !displayed_groups.contains(name) {
            // Extract the group name (everything before the first dot)
            if let Some(dot_pos) = name.find('.') {
                let group_name = &name[..dot_pos];
                let task_name = &name[dot_pos + 1..];
                task_groups
                    .entry(group_name.to_string())
                    .or_default()
                    .push((task_name.to_string(), task));
            }
        }
    }

    // Display the grouped tasks
    for (group_name, tasks) in &task_groups {
        println!("  {group_name}");
        for (task_name, task) in tasks {
            if verbose {
                match &task.description {
                    Some(desc) => println!("    {task_name}: {desc}"),
                    None => println!("    {task_name}"),
                }
            } else {
                println!("    {task_name}");
            }
        }
    }
    Ok(())
}

async fn execute_task(
    _config: std::sync::Arc<cuenv_config::Config>,
    environment: Option<String>,
    capabilities: Vec<String>,
    task_name: String,
    task_args: Vec<String>,
    audit: bool,
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
        .load_env_with_options(&current_dir, env_name, caps, None)
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
        let status = if audit {
            executor
                .execute_task_with_audit(&actual_task_name, &actual_args)
                .await?
        } else {
            executor
                .execute_task(&actual_task_name, &actual_args)
                .await?
        };
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

async fn execute_command(
    _config: std::sync::Arc<cuenv_config::Config>,
    environment: Option<String>,
    capabilities: Vec<String>,
    command: String,
    args: Vec<String>,
    audit: bool,
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
        .load_env_with_options(&current_dir, env_name, caps, None)
        .await?;

    if audit {
        use cuenv_security::AccessRestrictions;
        let _restrictions = AccessRestrictions::default();

        println!("🔍 Running command in audit mode...");
        println!("⚠️  Basic audit mode - run with task definition for full system call monitoring");
        let status = env_manager.run_command(&command, &args)?;
        std::process::exit(status);
    } else {
        let status = env_manager.run_command(&command, &args)?;
        std::process::exit(status);
    }
}
