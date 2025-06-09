use anyhow::Result;
use clap::{Parser, Subcommand};
use cuenv::{directory::DirectoryManager, env_manager::EnvManager, shell_hook::ShellHook};
use std::env;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cuenv")]
#[command(about = "A direnv alternative using CUE files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn get_current_shell() -> String {
    #[cfg(windows)]
    {
        // On Windows, check COMSPEC or default to PowerShell
        env::var("COMSPEC").ok()
            .and_then(|s| std::path::Path::new(&s).file_stem())
            .and_then(|s| s.to_str())
            .map(|s| {
                match s.to_lowercase().as_str() {
                    "cmd" => "cmd",
                    "powershell" => "powershell",
                    _ => "powershell"
                }
            })
            .unwrap_or("powershell")
            .to_string()
    }
    
    #[cfg(not(windows))]
    {
        env::var("SHELL").ok()
            .and_then(|s| s.split('/').last().map(String::from))
            .unwrap_or_else(|| "bash".to_string())
    }
}

#[derive(Subcommand)]
enum Commands {
    Load {
        #[arg(short, long)]
        directory: Option<PathBuf>,
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
        Some(Commands::Load { directory }) => {
            let dir = directory.unwrap_or_else(|| env::current_dir().unwrap());
            let mut env_manager = EnvManager::new();
            env_manager.load_env(&dir)?;
            
            let shell = get_current_shell();
            
            print!("{}", env_manager.export_for_shell(&shell)?);
        }
        Some(Commands::Unload) => {
            let env_manager = EnvManager::new();
            env_manager.unload_env()?;
            
            let shell = get_current_shell();
            
            print!("{}", env_manager.export_for_shell(&shell)?);
        }
        Some(Commands::Status) => {
            let env_manager = EnvManager::new();
            env_manager.print_env_diff()?;
        }
        Some(Commands::Hook { shell }) => {
            let current_dir = env::current_dir()?;
            print!("{}", ShellHook::generate_hook_output(&shell, &current_dir)?);
        }
        Some(Commands::Init { shell }) => {
            print!("{}", ShellHook::generate_hook(&shell)?);
        }
        Some(Commands::Allow { directory }) => {
            println!("Allowing directory: {}", directory.display());
        }
        Some(Commands::Deny { directory }) => {
            println!("Denying directory: {}", directory.display());
        }
        Some(Commands::Run { command, args }) => {
            let current_dir = env::current_dir()?;
            let mut env_manager = EnvManager::new();
            env_manager.load_env(&current_dir)?;
            
            // Execute the command with the loaded environment
            let status = env_manager.run_command(&command, &args)?;
            
            // Exit with the same status code as the child process
            std::process::exit(status);
        }
        None => {
            let dir_manager = DirectoryManager::new();
            let current_dir = DirectoryManager::get_current_directory()?;
            
            if dir_manager.should_load_env(&current_dir) {
                let mut env_manager = EnvManager::new();
                env_manager.load_env(&current_dir)?;
                println!("cuenv: loading env.cue");
            } else {
                println!("cuenv: no env.cue found in current directory");
            }
        }
    }

    Ok(())
}