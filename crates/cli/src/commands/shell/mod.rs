use crate::directory::DirectoryManager;
use crate::platform::{PlatformOps, Shell};
use clap::Subcommand;
use cuenv_core::{Result, CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR, ENV_CUE_FILENAME};
use cuenv_env::{EnvManager, StateManager};
use cuenv_shell::{ShellHook, ShellType};
use cuenv_utils::sync::env::InstanceLock;
use std::env;
use std::path::PathBuf;

// Import the platform-specific implementation
#[cfg(unix)]
use crate::platform::UnixPlatform as Platform;
#[cfg(windows)]
use crate::platform::WindowsPlatform as Platform;

#[derive(Subcommand)]
pub enum ShellCommands {
    /// Generate shell hook for automatic environment loading
    Init {
        /// Shell type (bash, zsh, fish, etc.)
        shell: String,
    },
    /// Manually load environment from current directory
    Load {
        #[arg(short, long)]
        directory: Option<PathBuf>,

        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,
    },
    /// Manually unload current environment
    Unload,
    /// Generate shell hook for current directory
    Hook {
        /// Shell name (defaults to current shell)
        shell: Option<String>,
    },
}

impl ShellCommands {
    pub async fn execute(self) -> Result<()> {
        match self {
            ShellCommands::Init { shell } => match ShellHook::generate_hook(&shell) {
                Ok(output) => {
                    print!("{output}");
                    Ok(())
                }
                Err(e) => Err(cuenv_core::Error::configuration(format!(
                    "Failed to generate shell hook: {e}"
                ))),
            },
            ShellCommands::Load {
                directory,
                environment,
                capabilities,
            } => {
                let _lock = InstanceLock::acquire()?;

                let dir = match directory {
                    Some(d) => d,
                    None => env::current_dir().map_err(|e| {
                        cuenv_core::Error::file_system(".", "get current directory", e)
                    })?,
                };
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
                    .load_env_with_options(&dir, env_name, caps, None)
                    .await?;

                let shell = Platform::get_current_shell()
                    .unwrap_or(Shell::Bash)
                    .as_str();

                match env_manager.export_for_shell(shell) {
                    Ok(output) => {
                        print!("{output}");
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            ShellCommands::Unload => {
                let _lock = InstanceLock::acquire()?;

                let mut env_manager = EnvManager::new();
                env_manager.unload_env()?;

                let shell = Platform::get_current_shell()
                    .unwrap_or(Shell::Bash)
                    .as_str();

                match env_manager.export_for_shell(shell) {
                    Ok(output) => {
                        print!("{output}");
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            ShellCommands::Hook { shell } => {
                // Set environment variable to indicate we're in shell hook mode
                env::set_var("CUENV_SHELL_HOOK", "1");

                let shell_type = match shell {
                    Some(s) => ShellType::from_name(&s),
                    None => {
                        if let Some(arg0) = env::args().next() {
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
                        }
                    }
                };

                let shell_impl = shell_type.as_shell();
                let current_dir = env::current_dir()?;

                if StateManager::should_unload(&current_dir) {
                    if let Ok(Some(diff)) = StateManager::get_diff() {
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
                    StateManager::unload().await.map_err(|e| {
                        cuenv_core::Error::configuration(format!("Failed to unload state: {e}"))
                    })?;
                } else if current_dir.join(ENV_CUE_FILENAME).exists() {
                    let dir_manager = DirectoryManager::new();
                    if dir_manager
                        .is_directory_allowed(&current_dir)
                        .unwrap_or(false)
                    {
                        if StateManager::files_changed() || StateManager::should_load(&current_dir)
                        {
                            let mut env_manager = EnvManager::new();
                            if let Err(e) = env_manager.load_env(&current_dir).await {
                                eprintln!("# cuenv: failed to load environment: {e}");
                            } else if let Ok(Some(diff)) = StateManager::get_diff() {
                                for (key, value) in diff.added_or_changed() {
                                    println!("{}", shell_impl.export(key, value));
                                }
                                for key in diff.removed() {
                                    println!("{}", shell_impl.unset(key));
                                }
                            }
                        }
                    } else {
                        eprintln!(
                            "# cuenv: Directory not allowed. Run 'cuenv allow {}' to allow this directory.",
                            current_dir.display()
                        );
                    }
                }
                Ok(())
            }
        }
    }
}
