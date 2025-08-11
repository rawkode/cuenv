//! Write-Ahead Log for crash recovery
//!
//! This module provides atomic operations and crash recovery
//! through a write-ahead log mechanism.

mod append;
mod operations;
mod replay;
mod rotation;
mod writer;

pub use operations::{WalEntry, WalOperation};
pub use writer::WriteAheadLog;
