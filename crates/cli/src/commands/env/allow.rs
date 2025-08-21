use crate::directory::DirectoryManager;
use cuenv_core::{Result, ENV_CUE_FILENAME};
use cuenv_env::EnvManager;
use std::{env, path::PathBuf};

pub async fn execute(directory: PathBuf) -> Result<()> {
    let dir_manager = DirectoryManager::new();
    let abs_dir = if directory.is_absolute() {
        directory
    } else {
        env::current_dir()
            .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?
            .join(directory)
    };
    dir_manager.allow_directory(&abs_dir)?;
    tracing::info!("✓ Allowed directory: {}", abs_dir.display());

    // If there's an env.cue file in the allowed directory, load it (which will execute hooks)
    if abs_dir.join(ENV_CUE_FILENAME).exists() {
        let mut env_manager = EnvManager::new();
        match env_manager.load_env(&abs_dir).await {
            Ok(_) => {
                tracing::info!("✓ Loaded environment and executed hooks");
            }
            Err(e) => {
                tracing::error!("⚠ Failed to load environment: {e}");
            }
        }
    }

    Ok(())
}
