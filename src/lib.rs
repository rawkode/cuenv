#![allow(clippy::question_mark)]
#![allow(clippy::await_holding_lock)]
#![allow(clippy::manual_map)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_strip)]
#![allow(clippy::get_first)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::io_other_error)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::derivable_impls)]

pub mod access_restrictions;
pub mod access_restrictions_builder;
pub mod async_runtime;
pub mod atomic_file;
pub mod audit;
pub mod cache;
pub mod command_executor;
pub mod core;
pub mod cue_cache;
pub mod cue_parser;
pub mod directory;
pub mod env_diff;
pub mod env_manager;
pub mod file_times;
pub mod gzenv;
pub mod hook_manager;
pub mod memory;
pub mod output_filter;
pub mod platform;
<<<<<<< HEAD
pub mod rate_limit;
// pub mod remote_cache;  // Temporarily disabled to focus on testing sourcing functionality
pub mod resilience;
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)
pub mod rate_limit;
pub mod remote_cache;
pub mod resilience;
=======
#[cfg(feature = "remote-cache")]
pub mod remote_cache;
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
pub mod resource_limits;
pub mod secrets;
pub mod security;
pub mod shell;
pub mod shell_hook;
pub mod state;
pub mod task_executor;
pub mod task_executor_tui;
pub mod tracing;
pub mod tui;
pub mod utils;
pub mod xdg;

// Re-export commonly used items for backward compatibility
pub use core::constants;
pub use core::errors;
pub use core::types;
pub use utils::cleanup;
pub use utils::sync::env as sync_env;
