// Re-export command modules
pub mod commands;
pub mod completion;
pub mod directory;
// pub mod formatters;  // MOVED - Now in commands::task::formatters 
pub mod monorepo;
pub mod platform;

// Re-export commonly used types
pub use commands::Commands;
pub use directory::DirectoryManager;
// pub use formatters::{SimpleFormatterSubscriber, SpinnerFormatterSubscriber};  // MOVED to task module
