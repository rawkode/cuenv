//! Main system event enum

use super::{CacheEvent, DependencyEvent, EnvEvent, LogEvent, PipelineEvent, TaskEvent};
use serde::{Deserialize, Serialize};

/// Main event enum that encompasses all event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Task(TaskEvent),
    Pipeline(PipelineEvent),
    Cache(CacheEvent),
    Env(EnvEvent),
    Dependency(DependencyEvent),
    Log(LogEvent),
}
