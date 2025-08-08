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
pub mod config;
pub mod core;
pub mod directory;
pub mod discovery;
pub mod env;
pub mod file_times;
pub mod hooks;
pub mod output_filter;
pub mod platform;
pub mod secrets;
pub mod security;
pub mod shell;
pub mod shell_hook;
pub mod state;
pub mod task;
pub mod task_executor;
pub mod task_executor_tui;
pub mod tracing;
pub mod tui;
pub mod utils;

// Re-export commonly used items for backward compatibility
pub use core::constants;
pub use core::errors;
pub use core::types;
pub use utils::cleanup;
pub use utils::sync::env as sync_env;
