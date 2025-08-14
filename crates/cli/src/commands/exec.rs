use cuenv_config::Config;
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR};
use cuenv_env::manager::environment::SupervisorMode;
use cuenv_env::EnvManager;
use std::env;
use std::sync::Arc;

pub async fn execute(
    _config: Arc<Config>,
    environment: Option<String>,
    capabilities: Vec<String>,
    command: String,
    args: Vec<String>,
    _audit: bool,
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

    // Load environment using the same approach as task commands
    env_manager
        .load_env_with_options(
            &current_dir,
            env_name,
            caps,
            None,
            SupervisorMode::Synchronous,
        )
        .await?;

    // Execute the command in the prepared environment
    // Use run_command_with_current_env to include variables set by preload hooks
    let exit_code = env_manager.run_command_with_current_env(&command, &args)?;

    std::process::exit(exit_code);
}
