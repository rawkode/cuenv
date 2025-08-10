use crate::commands::Commands;
use crate::directory::DirectoryManager;
use cuenv_core::{Result, ENV_CUE_FILENAME};
use cuenv_env::EnvManager;
use cuenv_task::TaskServerProvider;
use std::env;
use std::path::PathBuf;

impl Commands {
    pub async fn execute(self) -> Result<()> {
        match self {
            Commands::Task { command } => command.execute().await,
            Commands::Env { command } => command.execute().await,
            Commands::Shell { command } => command.execute().await,
            Commands::Cache { command } => command.execute().await,
            Commands::Internal { command } => command.execute().await,

            Commands::Init { force } => crate::commands::init::execute(force).await,
            Commands::Discover {
                max_depth,
                load,
                dump,
            } => crate::commands::discover::execute(max_depth, load, dump).await,
            Commands::Completion { shell } => crate::completion::generate_completion(&shell),
            Commands::CompleteTasks => complete_tasks().await,
            Commands::CompleteEnvironments => complete_environments().await,
            Commands::CompleteHosts => complete_hosts().await,
            Commands::Mcp {
                transport,
                port,
                socket,
                allow_exec,
            } => handle_mcp_server(transport, port, socket, allow_exec).await,

            // Legacy aliases
            Commands::Run {
                environment,
                capabilities,
                task_name,
                task_args,
                audit,
                output,
                trace_output,
            } => {
                if let Some(task_name) = task_name {
                    let cmd = crate::commands::task::TaskCommands::Run {
                        environment,
                        capabilities,
                        task_name,
                        task_args,
                        audit,
                        output,
                        trace_output,
                    };
                    cmd.execute().await
                } else {
                    // No task name, list tasks with descriptions
                    let cmd = crate::commands::task::TaskCommands::List { verbose: true };
                    cmd.execute().await
                }
            }
            Commands::Exec {
                environment,
                capabilities,
                command,
                args,
                audit,
            } => {
                let cmd = crate::commands::task::TaskCommands::Exec {
                    environment,
                    capabilities,
                    command,
                    args,
                    audit,
                };
                cmd.execute().await
            }
        }
    }
}

async fn complete_tasks() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    let mut env_manager = EnvManager::new();
    if let Ok(()) = env_manager.load_env(&current_dir).await {
        let tasks = env_manager.list_tasks();
        for (name, _description) in tasks {
            println!("{name}");
        }
    }
    Ok(())
}

async fn complete_environments() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    let cue_file = current_dir.join(ENV_CUE_FILENAME);
    if !cue_file.exists() {
        return Ok(());
    }

    let content = match std::fs::read_to_string(&cue_file) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let mut in_environment_section = false;
    let mut brace_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("environment:") {
            in_environment_section = true;
            if trimmed.contains('{') {
                brace_count += 1;
            }
            continue;
        }

        if in_environment_section {
            for ch in line.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
            }

            if brace_count > 0 && trimmed.ends_with(':') && !trimmed.contains('{') {
                let env_name = trimmed.trim_end_matches(':').trim();
                if !env_name.is_empty() && !env_name.starts_with("//") {
                    println!("{env_name}");
                }
            }

            if brace_count <= 0 {
                in_environment_section = false;
                continue;
            }
        }
    }

    Ok(())
}

async fn complete_hosts() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    let mut env_manager = EnvManager::new();
    if let Ok(()) = env_manager.load_env(&current_dir).await {
        let tasks = env_manager.get_tasks();
        for task in tasks.values() {
            if let Some(security) = &task.security {
                if let Some(allowed_hosts) = &security.allowed_hosts {
                    for host in allowed_hosts {
                        println!("{host}");
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_mcp_server(
    transport: String,
    port: u16,
    socket: Option<PathBuf>,
    allow_exec: bool,
) -> Result<()> {
    use cuenv_core::Error;

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

    // Extract tasks from environment manager
    let internal_tasks = env_manager.get_tasks().clone();

    // Create the appropriate server based on transport
    let mut provider = match transport.as_str() {
        "stdio" => {
            println!("Starting cuenv MCP server (stdio transport)");
            println!("Transport: stdio");
            println!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            println!("Ready for MCP clients (like Claude Code)");

            TaskServerProvider::new_stdio(internal_tasks, allow_exec)
        }
        "unix" => {
            let socket_path = socket.unwrap_or_else(|| {
                tempfile::tempdir()
                    .map(|d| d.path().join("cuenv-mcp.sock"))
                    .unwrap_or_else(|_| PathBuf::from("/tmp/cuenv-mcp.sock"))
            });

            println!("Starting cuenv MCP server (Unix socket transport)");
            println!("Socket: {}", socket_path.display());
            println!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );

            TaskServerProvider::new_with_options(
                Some(socket_path),
                internal_tasks,
                allow_exec,
                false,
            )
        }
        "tcp" => {
            println!("Starting cuenv MCP server (TCP transport)");
            println!("Port: {}", port);
            println!(
                "Task execution: {}",
                if allow_exec { "enabled" } else { "read-only" }
            );
            println!("Note: TCP transport uses Unix socket internally - external TCP not implemented yet");

            // For TCP, we'll create a temporary socket and note the limitation
            let temp_socket = tempfile::tempdir()
                .map(|d| d.path().join("cuenv-mcp-tcp.sock"))
                .map_err(|e| {
                    Error::configuration(format!("Failed to create temp socket: {}", e))
                })?;

            TaskServerProvider::new_with_options(
                Some(temp_socket),
                internal_tasks,
                allow_exec,
                false,
            )
        }
        _ => {
            return Err(Error::configuration(format!(
                "Unsupported transport: {}. Use 'stdio', 'unix', or 'tcp'",
                transport
            )));
        }
    };

    // Start the server (this will block until interrupted)
    println!("Press Ctrl+C to stop the server");

    // Set up signal handling for graceful shutdown
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        result = provider.start() => {
            match result {
                Ok(()) => println!("MCP server stopped successfully"),
                Err(e) => {
                    eprintln!("MCP server error: {}", e);
                    return Err(e);
                }
            }
        }
        _ = ctrl_c => {
            println!("Received interrupt signal, stopping MCP server...");
            if let Err(e) = provider.shutdown().await {
                eprintln!("Error during shutdown: {}", e);
            }
        }
    }

    Ok(())
}
