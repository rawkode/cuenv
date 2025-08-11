//! Shell completion generation for cuenv

mod shells;

use cuenv_core::Result;
use shells::{bash, elvish, fish, powershell, zsh};

/// Generate shell completion script for the specified shell
pub fn generate_completion(shell: &str) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => bash::generate(),
        "zsh" => zsh::generate(),
        "fish" => fish::generate(),
        "powershell" | "pwsh" => powershell::generate(),
        "elvish" => elvish::generate(),
        _ => {
            eprintln!("Unsupported shell: {shell}");
            eprintln!("Supported shells: bash, zsh, fish, powershell, elvish");
            std::process::exit(1);
        }
    }
}
