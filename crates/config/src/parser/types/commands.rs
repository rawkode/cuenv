//! Command configuration types

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub capabilities: Option<Vec<String>>,
}
