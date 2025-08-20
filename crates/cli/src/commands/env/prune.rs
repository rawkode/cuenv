use crate::platform::{PlatformOps, Shell};
use cuenv_core::Result;
use cuenv_env::StateManager;
use cuenv_shell::ShellType;
use std::env;

// Import the platform-specific implementation
#[cfg(unix)]
use crate::platform::UnixPlatform as Platform;
#[cfg(windows)]
use crate::platform::WindowsPlatform as Platform;

pub async fn execute() -> Result<()> {
    // Get the diff before unloading to generate cleanup shell commands
    if let Ok(Some(diff)) = StateManager::get_diff() {
        // Detect shell type
        let shell_type = if let Some(arg0) = env::args().next() {
            ShellType::detect_from_arg(&arg0)
        } else {
            match Platform::get_current_shell() {
                Ok(Shell::Bash) => ShellType::Bash,
                Ok(Shell::Zsh) => ShellType::Zsh,
                Ok(Shell::Fish) => ShellType::Fish,
                Ok(Shell::Pwsh) => ShellType::PowerShell,
                Ok(Shell::Cmd) => ShellType::Cmd,
                _ => ShellType::Bash,
            }
        };

        let shell_impl = shell_type.as_shell();

        // Output shell commands to unset environment variables
        eprintln!("# cuenv: Generating shell commands to clean up environment variables");
        for key in diff.removed() {
            println!("{}", shell_impl.unset(key));
        }
        for (key, _) in diff.added_or_changed() {
            if diff.prev.contains_key(key) {
                if let Some(orig_value) = diff.prev.get(key) {
                    println!("{}", shell_impl.export(key, orig_value));
                }
            } else {
                println!("{}", shell_impl.unset(key));
            }
        }
    }

    // Unload any stale state
    StateManager::unload().await?;
    eprintln!("✓ Pruned stale environment state");
    Ok(())
}
