use crate::commands::Commands;
use cuenv_config::Config;
use cuenv_core::Result;
use std::sync::Arc;

impl Commands {
    pub async fn execute(self, config: Arc<Config>) -> Result<()> {
        match self {
            // Handle the simplified task command
            Commands::Task {
                task_or_group,
                args,
                environment,
                capabilities,
                audit,
                verbose,
                output,
                trace_output,
            } => {
                crate::commands::task::execute_task_command(
                    Arc::clone(&config),
                    task_or_group,
                    args,
                    environment,
                    capabilities,
                    audit,
                    verbose,
                    output,
                    trace_output,
                )
                .await
            }
            Commands::Env { command } => command.execute().await,
            Commands::Shell { command } => command.execute().await,
            Commands::Cache { command } => command.execute().await,
            Commands::Internal { command } => command.execute().await,

            Commands::Init { force } => crate::commands::init::execute(config, force).await,
            Commands::Discover {
                max_depth,
                load,
                dump,
            } => crate::commands::discover::execute(config, max_depth, load, dump).await,
            Commands::Completion { shell } => crate::completion::generate_completion(&shell),
            Commands::CompleteTasks => complete_tasks(config).await,
            Commands::CompleteEnvironments => complete_environments(config).await,
            Commands::CompleteHosts => complete_hosts().await,
            Commands::Mcp {
                transport,
                port,
                socket,
                allow_exec,
            } => crate::commands::mcp::execute(config, transport, port, socket, allow_exec).await,
        }
    }
}

async fn complete_tasks(config: Arc<Config>) -> Result<()> {
    // Use config to get tasks
    let tasks = config.get_tasks();
    for name in tasks.keys() {
        println!("{name}");
    }
    Ok(())
}

async fn complete_environments(_config: Arc<Config>) -> Result<()> {
    // Use config to get environments if available
    // For now, just return Ok
    Ok(())
}

async fn complete_hosts() -> Result<()> {
    // Complete hosts - doesn't need config
    Ok(())
}
