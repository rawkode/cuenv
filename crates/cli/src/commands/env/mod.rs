use clap::Subcommand;
use std::path::PathBuf;

mod allow;
mod deny;
mod export;
mod prune;
mod status;

#[derive(Subcommand)]
pub enum EnvCommands {
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

    /// Export environment variables for the current directory
    Export {
        /// Shell format (defaults to current shell)
        #[arg(short, long)]
        shell: Option<String>,

        /// Export all system environment variables, not just loaded ones
        #[arg(long)]
        all: bool,
    },

    /// Prune stale environment state
    Prune,
}

impl EnvCommands {
    pub async fn execute(self) -> cuenv_core::Result<()> {
        match self {
            EnvCommands::Allow { directory } => allow::execute(directory).await,
            EnvCommands::Deny { directory } => deny::execute(directory).await,
            EnvCommands::Status => status::execute().await,
            EnvCommands::Export { shell, all } => export::execute(shell, all).await,
            EnvCommands::Prune => prune::execute().await,
        }
    }
}
