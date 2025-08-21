use crate::platform::{PlatformOps, Shell};
use cuenv_core::{Result, ENV_CUE_FILENAME};
use cuenv_env::EnvManager;
use cuenv_shell::ShellType;
use std::env;

// Import the platform-specific implementation
#[cfg(unix)]
use crate::platform::UnixPlatform as Platform;
#[cfg(windows)]
use crate::platform::WindowsPlatform as Platform;

pub async fn execute(shell: Option<String>, all: bool) -> Result<()> {
    let shell_type = match shell {
        Some(s) => ShellType::from_name(&s),
        None => match Platform::get_current_shell() {
            Ok(Shell::Bash) => ShellType::Bash,
            Ok(Shell::Zsh) => ShellType::Zsh,
            Ok(Shell::Fish) => ShellType::Fish,
            Ok(Shell::Pwsh) => ShellType::PowerShell,
            Ok(Shell::Cmd) => ShellType::Cmd,
            _ => ShellType::Bash,
        },
    };

    let shell_impl = shell_type.as_shell();

    if all {
        // Export all system environment variables
        for (key, value) in env::vars() {
            tracing::info!("{}", shell_impl.export(&key, &value));
        }
    } else {
        // Export only the loaded environment from env.cue
        let current_dir = env::current_dir()
            .map_err(|e| cuenv_core::Error::file_system(".", "get current directory", e))?;

        if current_dir.join(ENV_CUE_FILENAME).exists() {
            let mut env_manager = EnvManager::new();
            env_manager.load_env(&current_dir).await?;

            match env_manager.export_for_shell(shell_type.name()) {
                Ok(output) => tracing::info!("{output}"),
                Err(e) => return Err(e),
            }
        } else {
            tracing::error!("No {ENV_CUE_FILENAME} found in current directory");
            std::process::exit(1);
        }
    }

    Ok(())
}
