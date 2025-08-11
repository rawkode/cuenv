use clap::Subcommand;
use cuenv_core::{Error, Result};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum InternalCommands {
    /// Task Server Protocol implementation for devenv integration
    TaskProtocol {
        /// Task server executable to launch
        #[arg(long)]
        server: Option<String>,
        /// Directory to discover task servers
        #[arg(long)]
        discovery_dir: Option<PathBuf>,
        /// Task to run on external server
        #[arg(long)]
        run_task: Option<String>,
        /// List available tasks from servers
        #[arg(long)]
        list_tasks: bool,
        /// Start as a task server provider (expose cuenv tasks to external tools)
        #[arg(long)]
        serve: bool,
        /// Socket path for server mode
        #[arg(long)]
        socket: Option<PathBuf>,
        /// Export cuenv tasks as JSON for static consumption
        #[arg(long)]
        export_json: bool,
    },
}

impl InternalCommands {
    pub async fn execute(self) -> Result<()> {
        match self {
            InternalCommands::TaskProtocol {
                server,
                discovery_dir,
                run_task,
                list_tasks,
                serve,
                socket,
                export_json,
            } => {
                handle_task_protocol(
                    &server,
                    &discovery_dir,
                    &run_task,
                    list_tasks,
                    serve,
                    &socket,
                    export_json,
                )
                .await
            }
        }
    }
}

async fn handle_task_protocol(
    server: &Option<String>,
    discovery_dir: &Option<PathBuf>,
    run_task: &Option<String>,
    list_tasks: bool,
    serve: bool,
    socket: &Option<PathBuf>,
    export_json: bool,
) -> Result<()> {
    use cuenv_task::TaskServerManager;
    use std::collections::HashMap;

    // Create socket directory in temp
    let socket_dir = tempfile::tempdir().map_err(|e| {
        Error::configuration(format!("Failed to create temp socket directory: {}", e))
    })?;

    let mut manager = TaskServerManager::new(socket_dir.path().to_path_buf());

    // Add servers based on command line options
    let mut all_tasks = Vec::new();

    if let Some(server_executable) = server {
        // Launch a single server
        let server_name = std::path::Path::new(server_executable)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");

        match manager.add_server(server_executable, server_name).await {
            Ok(tasks) => {
                all_tasks.extend(tasks);
                println!("Connected to task server: {}", server_executable);
            }
            Err(e) => {
                eprintln!(
                    "Failed to connect to task server {}: {}",
                    server_executable, e
                );
                return Err(e);
            }
        }
    }

    if let Some(discovery_path) = discovery_dir {
        // Discover servers from directory
        match manager.discover_servers(discovery_path).await {
            Ok(tasks) => {
                let task_count = tasks.len();
                all_tasks.extend(tasks);
                println!(
                    "Discovered {} task servers from {}",
                    task_count,
                    discovery_path.display()
                );
            }
            Err(e) => {
                eprintln!("Failed to discover task servers: {}", e);
                return Err(e);
            }
        }
    }

    if list_tasks {
        // List all available tasks from servers
        if all_tasks.is_empty() {
            println!("No tasks available from task servers");
        } else {
            println!("Available tasks from external servers:");
            for task in &all_tasks {
                if let Some(description) = &task.description {
                    println!("  {}: {}", task.name, description);
                } else {
                    println!("  {}", task.name);
                }
            }
        }
    }

    if let Some(task_name) = run_task {
        // Run a specific task
        if all_tasks.iter().any(|t| t.name == *task_name) {
            println!("Running task: {}", task_name);

            let inputs = HashMap::new(); // TODO: Accept inputs from CLI
            let outputs = HashMap::new(); // TODO: Accept outputs from CLI

            match manager.run_task(task_name, inputs, outputs).await {
                Ok(exit_code) => {
                    if exit_code == 0 {
                        println!("Task '{}' completed successfully", task_name);
                    } else {
                        println!("Task '{}' failed with exit code {}", task_name, exit_code);
                        std::process::exit(exit_code);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to run task '{}': {}", task_name, e);
                    return Err(e);
                }
            }
        } else {
            eprintln!("Task '{}' not found", task_name);
            eprintln!("Available tasks:");
            for task in &all_tasks {
                eprintln!("  - {}", task.name);
            }
            std::process::exit(1);
        }
    }

    if serve {
        use crate::directory::DirectoryManager;
        use cuenv_env::EnvManager;
        use cuenv_task::TaskServerProvider;

        // Get current directory and load environment tasks
        let current_dir = match DirectoryManager::get_current_directory() {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to get current directory: {e}"
                )));
            }
        };

        let mut env_manager = EnvManager::new();
        env_manager.load_env(&current_dir).await?;

        // Create config from environment manager data
        use cuenv_config::{Config, ParseResult, RuntimeOptions};
        use std::collections::HashMap;
        use std::sync::Arc;

        let parse_result = ParseResult {
            variables: env_manager.get_cue_vars().clone(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks: env_manager.get_tasks().clone(),
            hooks: HashMap::new(),
        };

        let config = Arc::new(Config::new(
            current_dir.clone(),
            None, // no env file for internal command
            parse_result,
            RuntimeOptions::default(),
        ));

        // Determine socket path
        let socket_path = socket.clone().unwrap_or_else(|| {
            socket_dir
                .path()
                .join(format!("cuenv-{}.sock", std::process::id()))
        });

        println!(
            "Starting task server provider on socket: {}",
            socket_path.display()
        );

        // Create and start provider
        let mut provider = TaskServerProvider::new_with_options(
            Some(socket_path.clone()),
            config,
            false, // Don't allow execution by default for security
            false, // Not a subprocess
        );

        // Start the provider (blocks until shutdown)
        provider.start().await?;

        println!("Task server provider started successfully");
    }

    if export_json {
        use crate::directory::DirectoryManager;
        use cuenv_env::EnvManager;

        // Get current directory and load environment tasks
        let current_dir = match DirectoryManager::get_current_directory() {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to get current directory: {e}"
                )));
            }
        };

        let mut env_manager = EnvManager::new();
        env_manager.load_env(&current_dir).await?;

        // Extract and serialize tasks
        let tasks = env_manager.get_tasks();
        let json = serde_json::to_string_pretty(&tasks)?;
        println!("{}", json);
    }

    fn should_show_usage(
        serve: bool,
        export_json: bool,
        server: &Option<String>,
        discovery_dir: &Option<PathBuf>,
        run_task: &Option<String>,
        list_tasks: bool,
    ) -> bool {
        !serve
            && !export_json
            && server.is_none()
            && discovery_dir.is_none()
            && run_task.is_none()
            && !list_tasks
    }

    // If no action specified, show usage
    if should_show_usage(
        serve,
        export_json,
        &server,
        &discovery_dir,
        &run_task,
        list_tasks,
    ) {
        println!("Task Server Protocol (TSP) - Dual-Modality Support");
        println!();
        println!("Consumer Mode (use external task servers):");
        println!("  cuenv internal task-protocol --server <executable> --list-tasks");
        println!("  cuenv internal task-protocol --discovery-dir <path> --list-tasks");
        println!("  cuenv internal task-protocol --server <executable> --run-task <task>");
        println!();
        println!("Provider Mode (expose cuenv tasks to external tools):");
        println!("  cuenv internal task-protocol --serve [--socket <path>]");
        println!("  cuenv internal task-protocol --export-json");
    }

    // Shutdown servers
    manager.shutdown().await?;

    Ok(())
}
