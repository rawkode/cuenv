//! Task Server Protocol implementation
//!
//! This module implements the devenv Task Server Protocol (TSP) - a JSON-RPC based
//! protocol that allows external tools to register tasks with cuenv through a
//! long-running server process.
//!
//! Protocol specification:
//! - External tools act as JSON-RPC servers exposing tasks via Unix domain sockets
//! - cuenv discovers servers by launching executables (like `myexecutable /tmp/socket.sock`)
//! - Communication uses JSON-RPC 2.0 with initialize and run methods
//! - Servers stream back log output and final results

// Core protocol types
mod types;
pub use types::{RunTaskResult, TaskDefinition};

// Client for consuming external task servers
mod client;
pub use client::TaskServerClient;

// Manager for multiple task server clients
mod manager;
pub use manager::TaskServerManager;

// MCP protocol support
mod mcp;

// Request handlers
mod handlers;
mod handlers_tasks;
mod handlers_execution;

// Provider for exposing cuenv tasks
mod provider;
mod provider_handlers;
pub use provider::TaskServerProvider;

// Unified manager combining client and provider
mod unified;
pub use unified::UnifiedTaskManager;

// Tests
#[cfg(test)]
mod tests;