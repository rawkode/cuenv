use cuenv_core::Result;
use cuenv_env::EnvManager;

pub async fn execute() -> Result<()> {
    let env_manager = EnvManager::new();
    env_manager.print_env_diff()?;
    Ok(())
}
