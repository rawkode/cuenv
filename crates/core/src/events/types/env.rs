//! Environment loading events

use serde::{Deserialize, Serialize};

/// Environment loading events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvEvent {
    /// Environment file loading started
    EnvLoading { path: String },
    /// Environment file loaded successfully
    EnvLoaded { path: String, var_count: usize },
    /// Environment file loading failed
    EnvLoadFailed { path: String, error: String },
    /// Environment variable changed
    EnvVarChanged { key: String, is_secret: bool },
}