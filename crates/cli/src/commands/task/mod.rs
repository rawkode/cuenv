use clap::Subcommand;
use cuenv_config::{CueParser, ParseOptions};
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR};
use cuenv_env::EnvManager;
use cuenv_task::TaskExecutor;
use std::env;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List available tasks
    #[command(visible_alias = "l")]
    List {
        /// Show task descriptions
        #[arg(short, long)]
        verbose: bool,
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
    pub async fn execute(self) -> Result<()> {
        match self {
            TaskCommands::List { verbose } => list_tasks(verbose).await,
            TaskCommands::Run {
                environment,
                capabilities,
                task_name,
                task_args,
                audit,
                output: _,
                trace_output: _,
            } => execute_task(environment, capabilities, task_name, task_args, audit).await,
            TaskCommands::Exec {
                environment,
                capabilities,
                command,
                args,
                audit,
            } => execute_command(environment, capabilities, command, args, audit).await,
        }
    }
}

async fn list_tasks(verbose: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;

    // Check if we're in a monorepo context
    if crate::monorepo::is_monorepo(&current_dir) {
        crate::monorepo::list_monorepo_tasks(&current_dir).await?;
        return Ok(());
    }

    // Only parse the CUE file to get task definitions
    let options = ParseOptions {
        environment: env::var(CUENV_ENV_VAR).ok(),
        capabilities: Vec::new(),
    };

    let parse_result = CueParser::eval_package_with_options(&current_dir, "env", &options)?;

    if parse_result.tasks.is_empty() {
        println!("No tasks defined in the CUE package");
    } else {
        println!("Available tasks:");
        for (name, task) in parse_result.tasks {
            if verbose {
                match task.description {
                    Some(desc) => println!("  {name}: {desc}"),
                    None => println!("  {name}"),
                }
            } else {
                println!("{name}");
            }
        }
    }
    Ok(())
}

async fn execute_task(
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

    // Check if this is a cross-package task reference OR a local task with cross-package dependencies
    let has_cross_package_deps = if let Some(task) = env_manager.get_task(&task_name) {
        task.dependencies
            .as_ref()
            .map(|deps| deps.iter().any(|d| d.contains(':')))
            .unwrap_or(false)
    } else {
        false
    };

    if (task_name.contains(':') || has_cross_package_deps)
        && crate::monorepo::is_monorepo(&current_dir)
    {
        // Handle cross-package task execution
        let status =
            crate::monorepo::execute_monorepo_task(&current_dir, &task_name, &task_args, audit)
                .await?;
        std::process::exit(status);
    } else if env_manager.get_task(&task_name).is_some() {
        // Execute the specified task
        let executor = TaskExecutor::new(env_manager, current_dir).await?;
        let status = if audit {
            executor
                .execute_task_with_audit(&task_name, &task_args)
                .await?
        } else {
            executor.execute_task(&task_name, &task_args).await?
        };
        std::process::exit(status);
    } else {
        eprintln!("Task '{}' not found", task_name);
        eprintln!("Run 'cuenv task list' to see available tasks");
        std::process::exit(1);
    }
}

async fn execute_command(
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
