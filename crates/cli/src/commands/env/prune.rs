use cuenv_core::Result;
use cuenv_env::StateManager;

pub async fn execute() -> Result<()> {
    // Unload any stale state
    StateManager::unload().await?;
    println!("✓ Pruned stale environment state");
    Ok(())
}
