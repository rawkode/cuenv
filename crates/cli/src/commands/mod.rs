use clap::Subcommand;
use std::path::PathBuf;

pub mod cache;
pub mod discover;
pub mod env;
pub mod exec;
pub mod init;
pub mod internal;
pub mod mcp;
pub mod shell;
pub mod task;

use self::cache::CacheCommands;
use self::env::EnvCommands;
use self::internal::InternalCommands;
use self::shell::ShellCommands;

#[derive(Subcommand)]
pub enum Commands {
    /// List or execute tasks
    #[command(visible_alias = "t")]
    Task {
        /// Task or group name (optional - lists all if not provided)
        task_or_group: Option<String>,

        /// Subtask name (if first arg is a group) or arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,

        /// Environment to use (e.g., dev, staging, production)
        #[arg(short = 'e', long = "env")]
        environment: Option<String>,

        /// Capabilities to enable (can be specified multiple times)
        #[arg(short = 'c', long = "capability")]
        capabilities: Vec<String>,

        /// Run in audit mode to see file and network access without restrictions
        #[arg(long)]
        audit: bool,

        /// Show detailed descriptions when listing
        #[arg(short, long)]
        verbose: bool,

        /// Output format for task execution (tui, simple, or spinner)
        #[arg(long, value_name = "FORMAT", default_value = "spinner")]
        output: String,

        /// Generate Chrome trace output file
        #[arg(long)]
        trace_output: bool,

        /// Display task dependency graph instead of executing
        #[arg(long)]
        graph: bool,
    },

    /// Manage environment configuration
    #[command(visible_alias = "e")]
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },

    /// Initialize a new env.cue file with example configuration
    Init {
        /// Force overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

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

    /// Execute a command with the loaded environment
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

    // Internal commands
    /// Internal completion helper - complete task names
    #[command(name = "_complete_tasks", hide = true)]
    CompleteTasks,

    /// Internal completion helper - complete environment names  
    #[command(name = "_complete_environments", hide = true)]
    CompleteEnvironments,

    /// Internal completion helper - complete allowed hosts
    #[command(name = "_complete_hosts", hide = true)]
    CompleteHosts,

    /// Internal task server protocol implementation (experimental)
    #[command(name = "internal", hide = true)]
    Internal {
        #[command(subcommand)]
        command: InternalCommands,
    },

    /// Start MCP (Model Context Protocol) server for Claude Code integration
    Mcp {
        /// Transport type (stdio, tcp, unix)
        #[arg(long, default_value = "stdio")]
        transport: String,

        /// TCP port (only for tcp transport)
        #[arg(long, default_value = "8765")]
        port: u16,

        /// Unix socket path (only for unix transport, defaults to temp)
        #[arg(long)]
        socket: Option<PathBuf>,

        /// Allow task execution (default: read-only)
        #[arg(long)]
        allow_exec: bool,
    },

    /// Internal preload supervisor (hidden from user)
    #[command(name = "supervisor", hide = true)]
    Supervisor {
        /// JSON-encoded hooks to execute
        #[arg(long)]
        hooks: String,
    },
}
