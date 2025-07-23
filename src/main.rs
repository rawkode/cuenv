use clap::{Parser, Subcommand};
use cuenv::errors::{Error, Result};
use cuenv::platform::{PlatformOps, Shell};
use cuenv::{directory::DirectoryManager, env_manager::EnvManager, shell_hook::ShellHook};
use std::env;
use std::path::PathBuf;

// Import the platform-specific implementation
#[cfg(unix)]
use cuenv::platform::UnixPlatform as Platform;
#[cfg(windows)]
use cuenv::platform::WindowsPlatform as Platform;

#[derive(Parser)]
#[command(name = "cuenv")]
#[command(about = "A direnv alternative using CUE files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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
    Unload,
    Status,
    Hook {
        shell: String,
    },
    Init {
        shell: String,
    },
    Allow {
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
    Deny {
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
    Run {
        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,

        /// Command to run
        command: String,

        /// Arguments to pass to the command
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Load {
            directory,
            environment,
            capabilities,
        }) => {
            let dir = match directory {
                Some(d) => d,
                None => match env::current_dir() {
                    Ok(d) => d,
                    Err(e) => {
                        return Err(Error::file_system(
                            PathBuf::from("."),
                            "get current directory",
                            e,
                        ))
                    }
                },
            };
            let mut env_manager = EnvManager::new();

            // Use environment variables as fallback if CLI args not provided
            let env_name = environment.or_else(|| env::var("CUENV_ENV").ok());

            let mut caps = capabilities;
            if caps.is_empty() {
                // Check for CUENV_CAPABILITIES env var (comma-separated)
                if let Ok(env_caps) = env::var("CUENV_CAPABILITIES") {
                    caps = env_caps
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }

            // Load environment with options
            match env_manager.load_env_with_options(&dir, env_name, caps, None) {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            let shell = Platform::get_current_shell()
                .unwrap_or(Shell::Bash)
                .as_str();

            match env_manager.export_for_shell(shell) {
                Ok(output) => print!("{}", output),
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Unload) => {
            let env_manager = EnvManager::new();
            match env_manager.unload_env() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            let shell = Platform::get_current_shell()
                .unwrap_or(Shell::Bash)
                .as_str();

            match env_manager.export_for_shell(shell) {
                Ok(output) => print!("{}", output),
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Status) => {
            let env_manager = EnvManager::new();
            match env_manager.print_env_diff() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Hook { shell }) => {
            let current_dir = match env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::file_system(
                        PathBuf::from("."),
                        "get current directory",
                        e,
                    ))
                }
            };
            match ShellHook::generate_hook_output(&shell, &current_dir) {
                Ok(output) => print!("{}", output),
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Init { shell }) => match ShellHook::generate_hook(&shell) {
            Ok(output) => print!("{}", output),
            Err(e) => return Err(e),
        },
        Some(Commands::Allow { directory }) => {
            println!("Allowing directory: {}", directory.display());
        }
        Some(Commands::Deny { directory }) => {
            println!("Denying directory: {}", directory.display());
        }
        Some(Commands::Run {
            environment,
            capabilities,
            command,
            args,
        }) => {
            let current_dir = match env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::file_system(
                        PathBuf::from("."),
                        "get current directory",
                        e,
                    ))
                }
            };
            let mut env_manager = EnvManager::new();

            // Use environment variables as fallback if CLI args not provided
            let env_name = environment.or_else(|| env::var("CUENV_ENV").ok());

            let mut caps = capabilities;
            if caps.is_empty() {
                // Check for CUENV_CAPABILITIES env var (comma-separated)
                if let Ok(env_caps) = env::var("CUENV_CAPABILITIES") {
                    caps = env_caps
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }

            // Load environment with options and command for inference
            match env_manager.load_env_with_options(&current_dir, env_name, caps, Some(&command)) {
                Ok(()) => {}
                Err(e) => return Err(e),
            }

            // Execute the command with the loaded environment
            let status = match env_manager.run_command(&command, &args) {
                Ok(s) => s,
                Err(e) => return Err(e),
            };

            // Exit with the same status code as the child process
            std::process::exit(status);
        }
        None => {
            let dir_manager = DirectoryManager::new();
            let current_dir = match DirectoryManager::get_current_directory() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::configuration(format!(
                        "failed to get current directory: {}",
                        e
                    )))
                }
            };

            if dir_manager.should_load_env(&current_dir) {
                let mut env_manager = EnvManager::new();
                match env_manager.load_env(&current_dir) {
                    Ok(()) => println!("cuenv: loading env.cue"),
                    Err(e) => return Err(e),
                }
            } else {
                println!("cuenv: no env.cue found in current directory");
            }
        }
    }

    Ok(())
}
