use cuenv_config::Config;
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR};
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

    env_manager
        .load_env_with_options(&current_dir, env_name, caps, None)
        .await?;

    // Wait for preload hooks to complete before executing the command
    env_manager.wait_for_preload_hooks().await?;

    // For exec, we just run the command with the loaded environment
    // No restrictions are applied - this is the simple pass-through mode
    let exit_code = env_manager.run_command(&command, &args)?;

    std::process::exit(exit_code);
}
