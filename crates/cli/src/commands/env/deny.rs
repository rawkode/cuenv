use crate::directory::DirectoryManager;
use cuenv_core::Result;
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
    dir_manager.deny_directory(&abs_dir)?;
    tracing::info!("✓ Denied directory: {}", abs_dir.display());
    Ok(())
}
