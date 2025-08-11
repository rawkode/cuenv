use cuenv_core::Result;
use cuenv_utils::FileTimes;
use std::collections::HashMap;

// Hooks module stub - these will be properly implemented when hooks are re-enabled
pub async fn execute_nix_flake_hook(
    _flake: &str,
    _cache: &crate::cache::EnvCache,
    _reload: bool,
) -> Result<(HashMap<String, String>, FileTimes)> {
    Ok((HashMap::new(), FileTimes::new()))
}

pub async fn execute_devenv_hook(
    _devenv: &str,
    _cache: &crate::cache::EnvCache,
    _reload: bool,
) -> Result<(HashMap<String, String>, FileTimes)> {
    Ok((HashMap::new(), FileTimes::new()))
}

pub async fn execute_source_hook(
    _exec: &str,
    _cache: Option<&crate::cache::EnvCache>,
) -> Result<(HashMap<String, String>, FileTimes)> {
    Ok((HashMap::new(), FileTimes::new()))
}
