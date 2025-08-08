use clap::{Parser, Subcommand};

use cuenv::cache::CacheMode;
use cuenv::constants::{CUENV_CAPABILITIES_VAR, CUENV_ENV_VAR, ENV_CUE_FILENAME};
use cuenv::errors::{Error, Result};
use cuenv::platform::{PlatformOps, Shell};
use cuenv::shell::ShellType;
use cuenv::state::StateManager;
use cuenv::utils::sync::env::InstanceLock;
use cuenv::{
    config::{CueParser, ParseOptions},
    directory::DirectoryManager,
    env::EnvManager,
    shell_hook::ShellHook,
    task_executor::TaskExecutor,
};
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
#[command(version)]
struct Cli {
    /// Cache mode (off, read, read-write, write)
    #[arg(long, value_parser = ["off", "read", "read-write", "write"])]
    cache: Option<String>,

    /// Enable or disable caching globally
    #[arg(long)]
    cache_enabled: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new env.cue file with example configuration
    Init {
        /// Force overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Allow cuenv to load environments in a directory
    Allow {
        #[arg(default_value = ".")]
        directory: PathBuf,
    },

    /// Deny cuenv from loading environments in a directory
    Deny {
        #[arg(default_value = ".")]
        directory: PathBuf,
    },

    /// Display current environment status and changes
    Status,
    /// Discover all CUE packages in the repository
    Discover {
        /// Maximum depth to search for env.cue files
        #[arg(long, default_value = "32")]
        max_depth: usize,
        /// Load and validate discovered packages
        #[arg(short, long)]
        load: bool,
        /// Dump the CUE values for each package
        #[arg(short, long)]
        dump: bool,
    },

    /// Run a task or command with the loaded environment
    Run {
        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,

        /// Task name to execute
        task_name: Option<String>,

        /// Arguments to pass to the task (after --)
        #[arg(last = true)]
        task_args: Vec<String>,

        /// Run in audit mode to see file and network access without restrictions
        #[arg(long)]
        audit: bool,

        /// Output format for task execution (tui, simple, or spinner)
        #[arg(long, value_name = "FORMAT", default_value = "tui")]
        output: String,

        /// Generate Chrome trace output file
        #[arg(long)]
        trace_output: bool,
    },
    /// Execute a command directly with the loaded environment
    Exec {
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

        /// Run in audit mode to see file and network access without restrictions
        #[arg(long)]
        audit: bool,
    },

    /// Export environment variables for the current directory
    Export {
        /// Shell format (defaults to current shell)
        shell: Option<String>,
    },

    /// Dump complete environment in shell format
    Dump {
        /// Shell format (defaults to current shell)
        shell: Option<String>,
    },

    /// Prune stale environment state
    Prune,

    /// Manage the task and environment cache
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Configure shell integration for automatic environment loading
    Shell {
        #[command(subcommand)]
        command: ShellCommands,
    },

    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completion for
        shell: String,
    },
    /// Internal completion helper - complete task names
    #[command(name = "_complete_tasks", hide = true)]
    CompleteTasks,
    /// Internal completion helper - complete environment names  
    #[command(name = "_complete_environments", hide = true)]
    CompleteEnvironments,
    /// Internal completion helper - complete allowed hosts
    #[command(name = "_complete_hosts", hide = true)]
    CompleteHosts,
}

#[derive(Subcommand)]
enum ShellCommands {
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

#[derive(Subcommand)]
enum CacheCommands {
    /// Clear all cache entries
    Clear,
    /// Show cache statistics
    Stats,
    /// Clean up stale cache entries
    Cleanup {
        /// Maximum age of cache entries to keep (in hours)
        #[arg(long, default_value = "168")]
        max_age_hours: u64,
    },
}

fn generate_completion(shell: &str) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => generate_bash_completion(),
        "zsh" => generate_zsh_completion(),
        "fish" => generate_fish_completion(),
        "powershell" | "pwsh" => generate_powershell_completion(),
        "elvish" => generate_elvish_completion(),
        _ => {
            eprintln!("Unsupported shell: {shell}");
            eprintln!("Supported shells: bash, zsh, fish, powershell, elvish");
            std::process::exit(1);
        }
    }
}

fn generate_bash_completion() -> Result<()> {
    let script = r#"
_cuenv_completion() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Main commands
    local commands="init status allow deny run exec export dump prune cache shell completion help"
    
    # Flags that take arguments
    case "${prev}" in
        -e|--env)
            # Complete environment names
            COMPREPLY=($(compgen -W "$(cuenv _complete_environments 2>/dev/null || echo 'development staging production')" -- ${cur}))
            return 0
            ;;
        -c|--capability)
            # Complete capability names
            COMPREPLY=($(compgen -W "network filesystem secrets" -- ${cur}))
            return 0
            ;;
        --shell)
            COMPREPLY=($(compgen -W "bash zsh fish powershell" -- ${cur}))
            return 0
            ;;
    esac
    
    # Complete subcommands
    if [[ ${COMP_CWORD} == 1 ]]; then
        COMPREPLY=($(compgen -W "${commands}" -- ${cur}))
        return 0
    fi
    
    # Handle task completion for 'run' command
    if [[ "${COMP_WORDS[1]}" == "run" ]] && [[ ${COMP_CWORD} == 2 ]]; then
        COMPREPLY=($(compgen -W "$(cuenv _complete_tasks 2>/dev/null)" -- ${cur}))
        return 0
    fi
    
    # Complete flags for all commands
    local flags="-h --help -V --version -e --env -c --capability --audit"
    COMPREPLY=($(compgen -W "${flags}" -- ${cur}))
}

complete -F _cuenv_completion cuenv
"#;
    print!("{script}");
    Ok(())
}

fn generate_zsh_completion() -> Result<()> {
    let script = r#"
#compdef cuenv

_cuenv() {
    local context state line
    typeset -A opt_args
    
    _arguments -C \
        '1: :_cuenv_commands' \
        '*::arg:->args' \
        '(-h --help)'{-h,--help}'[Print help]' \
        '(-V --version)'{-V,--version}'[Print version]' \
        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
        '--audit[Run in audit mode]'
    
    case $state in
        args)
            case $words[1] in
                run)
                    _arguments \
                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                        '--audit[Run in audit mode]' \
                        '1: :_cuenv_tasks'
                    ;;
                exec)
                    _arguments \
                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                        '--audit[Run in audit mode]' \
                        '1: :_command_names'
                    ;;
                completion)
                    _arguments \
                        '1: :(bash zsh fish powershell)'
                    ;;
            esac
            ;;
    esac
}

_cuenv_commands() {
    local commands
    commands=(
        'init:Initialize a new env.cue file'
        'status:Display current environment status'
        'allow:Allow cuenv in a directory'
        'deny:Deny cuenv in a directory'
        'run:Run a task or command with the environment'
        'exec:Execute a command with the environment'
        'export:Export environment variables for the current directory'
        'dump:Dump complete environment'
        'prune:Prune stale state'
        'cache:Cache management commands'
        'shell:Shell integration commands'
        'completion:Generate shell completion scripts'
        'help:Print help information'
    )
    _describe 'commands' commands
}

_cuenv_tasks() {
    local tasks
    tasks=(${(f)"$(cuenv _complete_tasks 2>/dev/null)"})
    _describe 'tasks' tasks
}

_cuenv_environments() {
    local environments
    environments=(${(f)"$(cuenv _complete_environments 2>/dev/null || echo 'development\nstageing\nproduction')"})
    _describe 'environments' environments
}

_cuenv_capabilities() {
    local capabilities
    capabilities=(
        'network:Network access capability'
        'filesystem:Filesystem access capability'
        'secrets:Secrets access capability'
    )
    _describe 'capabilities' capabilities
}

_cuenv "$@"
"#;
    print!("{script}");
    Ok(())
}

fn generate_fish_completion() -> Result<()> {
    let script = r#"
# Fish completion for cuenv

# Main commands
complete -c cuenv -f
complete -c cuenv -n '__fish_use_subcommand' -a 'init' -d 'Initialize a new env.cue file'
complete -c cuenv -n '__fish_use_subcommand' -a 'status' -d 'Display current environment status'
complete -c cuenv -n '__fish_use_subcommand' -a 'allow' -d 'Allow cuenv in a directory'
complete -c cuenv -n '__fish_use_subcommand' -a 'deny' -d 'Deny cuenv in a directory'
complete -c cuenv -n '__fish_use_subcommand' -a 'run' -d 'Run a task or command with the environment'
complete -c cuenv -n '__fish_use_subcommand' -a 'exec' -d 'Execute a command with the environment'
complete -c cuenv -n '__fish_use_subcommand' -a 'export' -d 'Export environment variables for the current directory'
complete -c cuenv -n '__fish_use_subcommand' -a 'dump' -d 'Dump complete environment'
complete -c cuenv -n '__fish_use_subcommand' -a 'prune' -d 'Prune stale state'
complete -c cuenv -n '__fish_use_subcommand' -a 'cache' -d 'Cache management commands'
complete -c cuenv -n '__fish_use_subcommand' -a 'shell' -d 'Shell integration commands'
complete -c cuenv -n '__fish_use_subcommand' -a 'completion' -d 'Generate shell completion scripts'
complete -c cuenv -n '__fish_use_subcommand' -a 'help' -d 'Print help information'

# Global flags
complete -c cuenv -s h -l help -d 'Print help'
complete -c cuenv -s V -l version -d 'Print version'
complete -c cuenv -s e -l env -d 'Environment to use' -xa '(cuenv _complete_environments 2>/dev/null; or echo -e "development\nstaging\nproduction")'
complete -c cuenv -s c -l capability -d 'Capabilities to enable' -xa 'network filesystem secrets'
complete -c cuenv -l audit -d 'Run in audit mode'

# Task completion for run command
complete -c cuenv -n '__fish_seen_subcommand_from run' -xa '(cuenv _complete_tasks 2>/dev/null)'

# Shell completion for completion command
complete -c cuenv -n '__fish_seen_subcommand_from completion' -xa 'bash zsh fish powershell'

# Cache subcommands
complete -c cuenv -n '__fish_seen_subcommand_from cache' -xa 'clear stats cleanup'
"#;
    print!("{script}");
    Ok(())
}

fn generate_elvish_completion() -> Result<()> {
    let script = r#"
# Elvish completion for cuenv

edit:complete:arg-completer[cuenv] = {|@words|
    fn complete-commands {
        put init status allow deny run exec export dump prune cache shell completion help
    }
    
    fn complete-tasks {
        try {
            cuenv _complete_tasks 2>/dev/null | each {|line| put $line }
        } catch {
            # Silent fail for completion
        }
    }
    
    fn complete-environments {
        try {
            cuenv _complete_environments 2>/dev/null | each {|line| put $line }
        } catch {
            put development staging production
        }
    }
    
    fn complete-capabilities {
        put network filesystem secrets
    }
    
    set @words = $words[1:]
    var n = (count $words)
    
    if (== $n 0) {
        complete-commands
        return
    }
    
    var cmd = $words[0]
    
    if (== $cmd run) {
        if (== $n 1) {
            complete-tasks
            return
        }
    } elif (== $cmd completion) {
        if (== $n 1) {
            put bash zsh fish powershell elvish
            return
        }
    }
    
    # Complete flags
    put -h --help -V --version -e --env -c --capability --audit
}
"#;
    print!("{script}");
    Ok(())
}

fn generate_powershell_completion() -> Result<()> {
    let script = r#"
# PowerShell completion for cuenv

Register-ArgumentCompleter -Native -CommandName cuenv -ScriptBlock {
    param($commandName, $wordToComplete, $cursorPosition)
    
    $commands = @(
        'init', 'status', 'allow', 'deny', 'run', 'exec',
        'export', 'dump', 'prune', 'cache', 'shell',
        'completion', 'help'
    )
    
    $flags = @('-h', '--help', '-V', '--version', '-e', '--env', '-c', '--capability', '--audit')
    
    # Parse the current command line
    $tokens = $wordToComplete -split '\s+'
    $lastToken = $tokens[-1]
    
    # If we're completing the first argument (command)
    if ($tokens.Count -le 2) {
        $commands | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
        return
    }
    
    # Get the command
    $command = $tokens[1]
    
    switch ($command) {
        'run' {
            if ($tokens.Count -eq 3) {
                # Complete task names
                try {
                    $tasks = cuenv _complete_tasks 2>$null
                    if ($tasks) {
                        $tasks -split "`n" | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
                            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
                        }
                    }
                } catch {}
            }
        }
        'completion' {
            @('bash', 'zsh', 'fish', 'powershell') | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
                [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
            }
        }
    }
    
    # Complete flags
    $flags | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterName', $_)
    }
}
"#;
    print!("{script}");
    Ok(())
}

async fn complete_tasks() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    let mut env_manager = EnvManager::new();
    if let Ok(()) = env_manager.load_env(&current_dir).await {
        let tasks = env_manager.list_tasks();
        for (name, _description) in tasks {
            println!("{name}");
        }
    }
    Ok(())
}

async fn complete_environments() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    // Try to extract environment names from env.cue file content
    if current_dir.join("env.cue").exists() {
        if let Ok(content) = std::fs::read_to_string(current_dir.join("env.cue")) {
            // Parse to find environment section
            let lines: Vec<&str> = content.lines().collect();
            let mut in_environment_section = false;
            let mut brace_count = 0;

            for line in lines {
                let trimmed = line.trim();

                // Look for "environment:" line (with or without opening brace)
                if trimmed.starts_with("environment:") {
                    in_environment_section = true;
                    // Count opening braces on this line
                    brace_count += trimmed.matches('{').count() as i32;
                    brace_count -= trimmed.matches('}').count() as i32;
                    continue;
                }

                if in_environment_section {
                    // Look for environment names BEFORE updating brace count
                    // We want to catch "dev: {" when brace_count is still 1
                    if brace_count == 1 && trimmed.contains(':') && trimmed.contains('{') {
                        if let Some(colon_pos) = trimmed.find(':') {
                            let env_name = trimmed[..colon_pos].trim();
                            // Only accept valid identifiers that don't start with uppercase (not types)
                            if !env_name.is_empty()
                                && env_name
                                    .chars()
                                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                                && !env_name.chars().next().unwrap_or('A').is_uppercase()
                            {
                                println!("{env_name}");
                            }
                        }
                    }

                    // Count braces to track nesting
                    brace_count += trimmed.matches('{').count() as i32;
                    brace_count -= trimmed.matches('}').count() as i32;

                    // If we're back to 0 braces, we've exited the environment section
                    if brace_count <= 0 {
                        in_environment_section = false;
                        continue;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn complete_hosts() -> Result<()> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()), // Silent fail for completion
    };

    let mut env_manager = EnvManager::new();
    if let Ok(()) = env_manager.load_env(&current_dir).await {
        let tasks = env_manager.get_tasks();
        for task in tasks.values() {
            if let Some(security) = &task.security {
                if let Some(allowed_hosts) = &security.allowed_hosts {
                    for host in allowed_hosts {
                        println!("{host}");
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Check if --output=tui is present before initializing tracing
    let args: Vec<String> = env::args().collect();
    let use_tui = args.windows(2).any(|w| w[0] == "--output" && w[1] == "tui")
        || !args.iter().any(|arg| arg == "--output"); // Default is tui

    // Only initialize tracing if not using TUI
    if !use_tui {
        if let Err(e) = cuenv::tracing::init() {
            eprintln!("Failed to initialize tracing: {e}");
            std::process::exit(1);
        }
    }

    // Initialize cleanup handling for proper resource management
    cuenv::cleanup::init_cleanup_handler();

    let cli = Cli::parse();

    // Apply CLI cache configuration if provided
    if let Some(cache_mode) = &cli.cache {
        let _mode = match cache_mode.as_str() {
            "off" => CacheMode::Off,
            "read" => CacheMode::Read,
            "read-write" => CacheMode::ReadWrite,
            "write" => CacheMode::Write,
            _ => {
                return Err(Error::configuration(format!(
                    "Invalid cache mode: {cache_mode}"
                )))
            }
        };

        // Set environment variable for cache mode (highest precedence)
        std::env::set_var("CUENV_CACHE", cache_mode);
    }

    if let Some(enabled) = cli.cache_enabled {
        // Set environment variable for cache enabled/disabled
        std::env::set_var("CUENV_CACHE_ENABLED", enabled.to_string());
    }

    match cli.command {
        Some(Commands::Init { force }) => {
            // Initialize a new env.cue file
            let current_dir = env::current_dir()?;
            let env_file = current_dir.join(ENV_CUE_FILENAME);

            if env_file.exists() && !force {
                eprintln!(
                    "Error: {} already exists. Use --force to overwrite.",
                    ENV_CUE_FILENAME
                );
                std::process::exit(1);
            }

            let template = r#"package main

import "cuenv.io/env"

// Define your environment configuration
environment: env.#Environment & {
    // Development environment
    dev: {
        variables: {
            NODE_ENV: {
                value: "development"
                description: "Node environment"
            }
            DEBUG: {
                value: "true"
                description: "Enable debug mode"
            }
        }
    }
    
    // Production environment
    production: {
        variables: {
            NODE_ENV: {
                value: "production"
                description: "Node environment"
            }
            DEBUG: {
                value: "false"
                description: "Disable debug mode"
            }
        }
    }
}

// Define tasks
tasks: env.#Tasks & {
    dev: {
        description: "Start development server"
        command: ["npm", "run", "dev"]
    }
    
    build: {
        description: "Build for production"
        command: ["npm", "run", "build"]
    }
    
    test: {
        description: "Run tests"
        command: ["npm", "test"]
    }
}
"#;

            std::fs::write(&env_file, template)?;
            println!("✓ Created {} with example configuration", ENV_CUE_FILENAME);
            println!("\nNext steps:");
            println!(
                "  1. Edit {} to customize your environment",
                ENV_CUE_FILENAME
            );
            println!(
                "  2. Run 'cuenv allow {}' to allow this directory",
                current_dir.display()
            );
            println!("  3. Add shell hook with 'eval \"$(cuenv shell init <shell>)\"'");
        }
        Some(Commands::Shell { command }) => {
            match command {
                ShellCommands::Init { shell } => match ShellHook::generate_hook(&shell) {
                    Ok(output) => print!("{output}"),
                    Err(e) => return Err(e),
                },
                ShellCommands::Load {
                    directory,
                    environment,
                    capabilities,
                } => {
                    // Same as Commands::Load
                    let _lock = match InstanceLock::acquire() {
                        Ok(lock) => lock,
                        Err(e) => {
                            return Err(Error::Configuration {
                                message: e.to_string(),
                            })
                        }
                    };

                    let dir = match directory {
                        Some(d) => d,
                        None => match env::current_dir() {
                            Ok(d) => d,
                            Err(e) => {
                                return Err(Error::file_system(
                                    PathBuf::from("."),
                                    "get current directory",
                                    e,
                                ));
                            }
                        },
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
                        Ok(output) => print!("{output}"),
                        Err(e) => return Err(e),
                    }
                }
                ShellCommands::Unload => {
                    // Same as Commands::Unload
                    let _lock = match InstanceLock::acquire() {
                        Ok(lock) => lock,
                        Err(e) => {
                            return Err(Error::Configuration {
                                message: e.to_string(),
                            })
                        }
                    };

                    let mut env_manager = EnvManager::new();
                    env_manager.unload_env()?;

                    let shell = Platform::get_current_shell()
                        .unwrap_or(Shell::Bash)
                        .as_str();

                    match env_manager.export_for_shell(shell) {
                        Ok(output) => print!("{output}"),
                        Err(e) => return Err(e),
                    }
                }
                ShellCommands::Hook { shell } => {
                    // Same as Commands::Hook
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
                                    Ok(Shell::PowerShell) => ShellType::PowerShell,
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
                            Error::configuration(format!("Failed to unload state: {e}"))
                        })?;
                    } else if current_dir.join(ENV_CUE_FILENAME).exists() {
                        let dir_manager = DirectoryManager::new();
                        if dir_manager
                            .is_directory_allowed(&current_dir)
                            .unwrap_or(false)
                        {
                            if StateManager::files_changed()
                                || StateManager::should_load(&current_dir)
                            {
                                let mut env_manager = EnvManager::new();
                                if let Err(e) = env_manager.load_env(&current_dir).await {
                                    eprintln!("# cuenv: failed to load environment: {e}");
                                } else {
                                    if let Ok(Some(diff)) = StateManager::get_diff() {
                                        for (key, value) in diff.added_or_changed() {
                                            println!("{}", shell_impl.export(key, value));
                                        }
                                        for key in diff.removed() {
                                            println!("{}", shell_impl.unset(key));
                                        }
                                    }
                                }
                            }
                        } else {
                            eprintln!("# cuenv: Directory not allowed. Run 'cuenv allow {}' to allow this directory.", current_dir.display());
                        }
                    }
                }
            }
        }
        Some(Commands::Status) => {
            let env_manager = EnvManager::new();
            match env_manager.print_env_diff() {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Discover {
            max_depth,
            load,
            dump,
        }) => {
            use cuenv::discovery::PackageDiscovery;

            let current_dir = std::env::current_dir().map_err(|e| Error::Configuration {
                message: format!("Failed to get current directory: {}", e),
            })?;

            let mut discovery = PackageDiscovery::new(max_depth);

            // If dump is requested, we need to load the packages
            let should_load = load || dump;

            match discovery.discover(&current_dir, should_load).await {
                Ok(packages) => {
                    if packages.is_empty() {
                        println!("No CUE packages found");
                    } else {
                        if dump {
                            // Dump mode: show full details for each package
                            for package in packages {
                                println!("═══════════════════════════════════════════════");
                                println!("Package: {}", package.name);
                                println!("Path: {}", package.path.display());

                                if let Some(ref result) = package.parse_result {
                                    println!("\nEnvironment Variables:");
                                    if result.variables.is_empty() {
                                        println!("  (none)");
                                    } else {
                                        for (key, value) in &result.variables {
                                            println!("  {}: {}", key, value);
                                        }
                                    }

                                    if !result.tasks.is_empty() {
                                        println!("\nTasks:");
                                        for (name, task) in &result.tasks {
                                            print!("  {}", name);
                                            if let Some(ref desc) = task.description {
                                                print!(" - {}", desc);
                                            }
                                            println!();
                                        }
                                    }

                                    if !result.commands.is_empty() {
                                        println!("\nCommands:");
                                        for (name, cmd) in &result.commands {
                                            print!("  {}", name);
                                            if let Some(ref caps) = cmd.capabilities {
                                                print!(" (capabilities: {})", caps.join(", "));
                                            }
                                            println!();
                                        }
                                    }

                                    if !result.hooks.is_empty() {
                                        println!("\nHooks:");
                                        for (hook_type, hooks) in &result.hooks {
                                            println!("  {}: {} hook(s)", hook_type, hooks.len());
                                        }
                                    }
                                } else {
                                    println!("\n[Failed to load package]");
                                }
                                println!();
                            }
                        } else {
                            // Normal mode: just list packages
                            println!("Discovered CUE packages:");
                            for package in packages {
                                if load && package.parse_result.is_some() {
                                    println!(
                                        "  {} -> {} [loaded]",
                                        package.name,
                                        package.path.display()
                                    );
                                } else {
                                    println!("  {} -> {}", package.name, package.path.display());
                                }
                            }
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Allow { directory }) => {
            let dir_manager = DirectoryManager::new();
            let abs_dir = if directory.is_absolute() {
                directory
            } else {
                env::current_dir()?.join(directory)
            };
            match dir_manager.allow_directory(&abs_dir) {
                Ok(()) => println!("✓ Allowed directory: {}", abs_dir.display()),
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Deny { directory }) => {
            let dir_manager = DirectoryManager::new();
            let abs_dir = if directory.is_absolute() {
                directory
            } else {
                env::current_dir()?.join(directory)
            };
            match dir_manager.deny_directory(&abs_dir) {
                Ok(()) => println!("✓ Denied directory: {}", abs_dir.display()),
                Err(e) => return Err(e),
            }
        }
        Some(Commands::Run {
            environment,
            capabilities,
            task_name,
            task_args,
            audit,
            output,
            trace_output,
        }) => {
            let current_dir = match env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::file_system(
                        PathBuf::from("."),
                        "get current directory",
                        e,
                    ));
                }
            };

            // Special case: if no task name is provided, just list available tasks
            // without loading the full environment (skip onEnter hooks)
            if task_name.is_none() {
                // Check if we're in a monorepo context
                if cuenv::monorepo::is_monorepo(&current_dir) {
                    cuenv::monorepo::list_monorepo_tasks(&current_dir).await?;
                    return Ok(());
                }

                // Only parse the CUE file to get task definitions
                let options = ParseOptions {
                    environment: environment.or_else(|| env::var(CUENV_ENV_VAR).ok()),
                    capabilities: Vec::new(),
                };

                let parse_result =
                    CueParser::eval_package_with_options(&current_dir, "env", &options)?;

                if parse_result.tasks.is_empty() {
                    println!("No tasks defined in the CUE package");
                } else {
                    println!("Available tasks:");
                    for (name, task) in parse_result.tasks {
                        match task.description {
                            Some(desc) => println!("  {name}: {desc}"),
                            None => println!("  {name}"),
                        }
                    }
                }
                return Ok(());
            }

            // For actual task execution, load the full environment
            let mut env_manager = EnvManager::new();

            // Use environment variables as fallback if CLI args not provided
            let env_name = environment.or_else(|| env::var(CUENV_ENV_VAR).ok());

            let mut caps = capabilities;
            if caps.is_empty() {
                // Check for CUENV_CAPABILITIES env var (comma-separated)
                if let Ok(env_caps) = env::var(CUENV_CAPABILITIES_VAR) {
                    caps = env_caps
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }

            // Load environment with options
            env_manager
                .load_env_with_options(&current_dir, env_name, caps, None)
                .await?;

            match task_name {
                Some(name) => {
                    // Check if this is a cross-package task reference
                    if name.contains(':') && cuenv::monorepo::is_monorepo(&current_dir) {
                        // Handle cross-package task execution
                        let status = cuenv::monorepo::execute_monorepo_task(
                            &current_dir,
                            &name,
                            &task_args,
                            audit,
                        )
                        .await?;
                        std::process::exit(status);
                    } else if env_manager.get_task(&name).is_some() {
                        // Execute the specified task
                        let executor = TaskExecutor::new(env_manager, current_dir).await?;
                        let status = if output == "simple" {
                            // Force simple output
                            use cuenv::task_executor_tui::TaskExecutorTui;
                            executor
                                .execute_with_simple_output(
                                    &[name.clone()],
                                    &task_args,
                                    audit,
                                    trace_output,
                                )
                                .await?
                        } else if output == "tui" {
                            // Use TUI mode
                            use cuenv::task_executor_tui::TaskExecutorTui;
                            executor
                                .execute_with_tui(&[name.clone()], &task_args, audit)
                                .await?
                        } else if output == "spinner" {
                            // Use spinner mode (Docker Compose-style)
                            use cuenv::task_executor_tui::TaskExecutorTui;
                            executor
                                .execute_with_spinner(&[name.clone()], &task_args, audit)
                                .await?
                        } else if audit {
                            executor.execute_task_with_audit(&name, &task_args).await?
                        } else {
                            executor.execute_task(&name, &task_args).await?
                        };
                        std::process::exit(status);
                    } else {
                        // Treat as direct command execution without restrictions
                        // For restrictions, use task definitions with security config
                        let mut args = vec![name];
                        args.extend(task_args);

                        // For direct command execution, use the first argument as command
                        if args.is_empty() {
                            return Err(Error::configuration("No command provided".to_string()));
                        }

                        let command = &args[0];
                        let command_args = &args[1..];

                        // Execute the command without restrictions for direct execution
                        let status = env_manager.run_command(command, command_args)?;
                        std::process::exit(status);
                    }
                }
                None => unreachable!("task_name is None should have been handled earlier"),
            }
        }
        Some(Commands::Exec {
            environment,
            capabilities,
            command,
            args,
            audit,
        }) => {
            let current_dir = match env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::file_system(
                        PathBuf::from("."),
                        "get current directory",
                        e,
                    ));
                }
            };
            let mut env_manager = EnvManager::new();

            // Use environment variables as fallback if CLI args not provided
            let env_name = environment.or_else(|| env::var(CUENV_ENV_VAR).ok());

            let mut caps = capabilities;
            if caps.is_empty() {
                // Check for CUENV_CAPABILITIES env var (comma-separated)
                if let Ok(env_caps) = env::var(CUENV_CAPABILITIES_VAR) {
                    caps = env_caps
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }

            // Load environment with options and command for inference
            env_manager
                .load_env_with_options(&current_dir, env_name, caps, None)
                .await?;

            // Execute the command with the loaded environment
            if audit {
                // For exec audit mode, create a temporary restriction object
                use cuenv::access_restrictions::AccessRestrictions;
                let _restrictions = AccessRestrictions::default();

                // Use the env_manager's run_command but with audit monitoring
                println!("🔍 Running command in audit mode...");

                // Create a simple audit by running the command and capturing output
                // For a more comprehensive audit, we'd need to integrate strace monitoring
                // into the env_manager's run_command method
                println!("⚠️  Basic audit mode - run with task definition for full system call monitoring");
                let status = env_manager.run_command(&command, &args)?;
                std::process::exit(status);
            } else {
                // Execute without restrictions for direct exec
                let status = env_manager.run_command(&command, &args)?;
                std::process::exit(status);
            }
        }
        Some(Commands::Export { shell }) => {
            let shell_type = match shell {
                Some(s) => ShellType::from_name(&s),
                None => match Platform::get_current_shell() {
                    Ok(Shell::Bash) => ShellType::Bash,
                    Ok(Shell::Zsh) => ShellType::Zsh,
                    Ok(Shell::Fish) => ShellType::Fish,
                    Ok(Shell::PowerShell) => ShellType::PowerShell,
                    Ok(Shell::Cmd) => ShellType::Cmd,
                    _ => ShellType::Bash,
                },
            };

            let shell_impl = shell_type.as_shell();

            // Output current cuenv state as exports
            if let Ok(Some(diff)) = StateManager::get_diff() {
                for (key, value) in &diff.next {
                    if !diff.prev.contains_key(key) || diff.prev.get(key) != Some(value) {
                        println!("{}", shell_impl.export(key, value));
                    }
                }
            } else {
                eprintln!("# No cuenv environment loaded");
            }
        }
        Some(Commands::Dump { shell }) => {
            let shell_type = match shell {
                Some(s) => ShellType::from_name(&s),
                None => match Platform::get_current_shell() {
                    Ok(Shell::Bash) => ShellType::Bash,
                    Ok(Shell::Zsh) => ShellType::Zsh,
                    Ok(Shell::Fish) => ShellType::Fish,
                    Ok(Shell::PowerShell) => ShellType::PowerShell,
                    Ok(Shell::Cmd) => ShellType::Cmd,
                    _ => ShellType::Bash,
                },
            };

            let shell_impl = shell_type.as_shell();

            // Dump entire environment
            let current_env: std::collections::HashMap<String, String> = env::vars().collect();
            println!("{}", shell_impl.dump(&current_env));
        }
        Some(Commands::Prune) => {
            // For now, just unload if there's state
            if StateManager::is_loaded() {
                StateManager::unload()
                    .await
                    .map_err(|e| Error::configuration(format!("Failed to unload state: {e}")))?;
                println!("Pruned cuenv state");
            } else {
                println!("No cuenv state to prune");
            }
        }
        Some(Commands::Cache { command }) => {
            let cache = cuenv::cache::new_cache(".cache").build_sync()?;

            match command {
                CacheCommands::Clear => match cache.clear() {
                    Ok(()) => println!("✓ Cache cleared successfully"),
                    Err(e) => {
                        eprintln!("Failed to clear cache: {e}");
                        return Err(e.into());
                    }
                },
                CacheCommands::Stats => {
                    let stats = cache.statistics()?;
                    println!("Cache Statistics:");
                    println!("  Hits: {}", stats.hits);
                    println!("  Misses: {}", stats.misses);
                    println!("  Writes: {}", stats.writes);
                    println!("  Errors: {}", stats.errors);
                    println!("  Removals: {}", stats.removals);
                    println!("  Entry count: {}", stats.entry_count);
                    println!("  Total bytes: {}", stats.total_bytes);
                    println!("  Max bytes: {}", stats.max_bytes);
                    println!("  Expired cleanups: {}", stats.expired_cleanups);
                    println!("  Stats since: {:?}", stats.stats_since);
                    if stats.compression_enabled {
                        println!(
                            "  Compression enabled: ratio {:.2}",
                            stats.compression_ratio
                        );
                    }
                }
                CacheCommands::Cleanup { max_age_hours: _ } => {
                    // The new cache has automatic cleanup, just trigger a clear for now
                    println!("Note: The new cache implementation performs automatic cleanup.");
                    println!("For immediate cleanup, use 'cuenv cache clear' instead.");
                }
            }
        }
        Some(Commands::Completion { shell }) => {
            generate_completion(&shell)?;
        }
        Some(Commands::CompleteTasks) => {
            complete_tasks().await?;
        }
        Some(Commands::CompleteEnvironments) => {
            complete_environments().await?;
        }
        Some(Commands::CompleteHosts) => {
            complete_hosts().await?;
        }
        None => {
            let current_dir = match DirectoryManager::get_current_directory() {
                Ok(d) => d,
                Err(e) => {
                    return Err(Error::configuration(format!(
                        "failed to get current directory: {e}"
                    )));
                }
            };

            let mut env_manager = EnvManager::new();
            match env_manager.load_env(&current_dir).await {
                Ok(()) => println!("cuenv: loaded CUE package from {}", current_dir.display()),
                Err(e) => {
                    eprintln!("cuenv: failed to load CUE package: {e}");
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}
