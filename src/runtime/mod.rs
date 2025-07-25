pub mod host;
pub mod nix;
pub mod docker;
pub mod podman;
pub mod buildkit;

#[cfg(test)]
mod tests;

use crate::cue_parser::{RuntimeConfig, RuntimeType, RuntimeTypeConfig};
use crate::errors::{Error, Result};
use async_trait::async_trait;
use std::path::Path;

/// Trait for runtime environment execution
#[async_trait]
pub trait RuntimeExecutor {
    /// Execute a command or script in the runtime environment
    async fn execute(
        &self,
        command: Option<&str>,
        script: Option<&str>,
        shell: Option<&str>,
        working_dir: &Path,
        env_vars: &std::collections::HashMap<String, String>,
        args: &[String],
    ) -> Result<i32>;

    /// Check if the runtime is available on the system
    fn is_available(&self) -> bool;

    /// Get the name of the runtime
    fn name(&self) -> &'static str;
}

/// Create a runtime executor based on configuration
pub fn create_runtime_executor(runtime: &RuntimeConfig) -> Result<Box<dyn RuntimeExecutor + Send + Sync>> {
    match runtime.runtime_type {
        RuntimeType::Host => Ok(Box::new(host::HostRuntime::new())),
        RuntimeType::Nix => {
            let config = match &runtime.config {
                Some(RuntimeTypeConfig::Nix(config)) => config.clone(),
                _ => Default::default(),
            };
            Ok(Box::new(nix::NixRuntime::new(config)))
        }
        RuntimeType::Docker => {
            let config = match &runtime.config {
                Some(RuntimeTypeConfig::Docker(config)) => config.clone(),
                _ => return Err(Error::configuration("Docker runtime requires image configuration".to_string())),
            };
            Ok(Box::new(docker::DockerRuntime::new(config)))
        }
        RuntimeType::Podman => {
            let config = match &runtime.config {
                Some(RuntimeTypeConfig::Podman(config)) => config.clone(),
                _ => return Err(Error::configuration("Podman runtime requires image configuration".to_string())),
            };
            Ok(Box::new(podman::PodmanRuntime::new(config)))
        }
        RuntimeType::Buildkit => {
            let config = match &runtime.config {
                Some(RuntimeTypeConfig::Buildkit(config)) => config.clone(),
                _ => return Err(Error::configuration("BuildKit runtime requires image configuration".to_string())),
            };
            Ok(Box::new(buildkit::BuildkitRuntime::new(config)))
        }
    }
}

/// Default runtime executor for host execution
pub fn default_runtime_executor() -> Box<dyn RuntimeExecutor + Send + Sync> {
    Box::new(host::HostRuntime::new())
}