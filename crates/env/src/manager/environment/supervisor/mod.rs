//! Preload hook supervisor that manages background hook execution

mod cache;
mod core;
mod execution;
mod utils;

#[cfg(test)]
mod tests;

pub use cache::CapturedEnvironment;
pub use core::{Supervisor, SupervisorMode};
pub use utils::get_cache_dir;
