use cuenv_cache::concurrent::action::ActionCache;
use cuenv_cache::config::CacheConfiguration;
use std::path::Path;

/// Context for task execution to reduce function parameter count
pub struct TaskExecutionContext<'a> {
    pub cache_config: &'a CacheConfiguration,
    pub working_dir: &'a Path,
    pub action_cache: &'a ActionCache,
    pub audit_mode: bool,
    pub capture_output: bool,
}
