//! Shell integration for cuenv
//!
//! This crate handles shell-specific functionality including hooks,
//! command generation, and cross-shell compatibility.

pub mod bash;
pub mod cmd;
pub mod elvish;
pub mod fish;
pub mod mod_shell;
pub mod murex;
pub mod pwsh;
pub mod shell_hook;
pub mod tcsh;
pub mod zsh;

pub use bash::*;
pub use cmd::*;
pub use elvish::*;
pub use fish::*;
pub use mod_shell::*;
pub use murex::*;
pub use pwsh::*;
pub use shell_hook::*;
pub use tcsh::*;
pub use zsh::*;
