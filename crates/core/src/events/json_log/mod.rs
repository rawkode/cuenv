//! JSON log event subscriber for structured logging

mod config;
mod error;
mod formatter;
mod rotation;
mod subscriber;
mod writer;

pub use config::{JsonLogConfig, JsonLogSubscriber};
pub use error::JsonLogError;
pub use subscriber::JsonLogEventSubscriber;

#[cfg(test)]
mod tests;
