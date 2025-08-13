//! Environment caching functionality for the supervisor

use cuenv_config::Hook;
use cuenv_core::Result;
use globset::Glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Represents captured environment from source hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedEnvironment {
    /// Environment variables to set
    pub env_vars: HashMap<String, String>,
    /// Hash of hook inputs that generated this environment
    pub input_hash: String,
    /// Timestamp when captured
    pub timestamp: u64,
}

/// Load cached environment from disk
pub fn load_cached_environment(
    cache_dir: &Path,
    input_hash: &str,
) -> Result<CapturedEnvironment> {
    let cache_file = cache_dir.join(format!("{input_hash}.json"));

    if !cache_file.exists() {
        return Err(cuenv_core::Error::configuration("Cache file not found"));
    }

    let content = fs::read_to_string(&cache_file)
        .map_err(|e| cuenv_core::Error::file_system(&cache_file, "read cache", e))?;

    serde_json::from_str(&content)
        .map_err(|e| cuenv_core::Error::configuration(format!("Failed to parse cache: {e}")))
}

/// Maximum size for cached environment (10MB)
const MAX_CACHE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum number of environment variables
const MAX_ENV_VARS: usize = 1000;

/// Save captured environment to cache with size limits
pub fn save_cached_environment(
    cache_dir: &Path,
    input_hash: &str,
    env_vars: HashMap<String, String>,
) -> Result<()> {
    // Check memory limits
    if env_vars.len() > MAX_ENV_VARS {
        return Err(cuenv_core::Error::configuration(format!(
            "Too many environment variables: {} (max: {MAX_ENV_VARS})",
            env_vars.len()
        )));
    }

    // Estimate memory usage
    let estimated_size: usize = env_vars
        .iter()
        .map(|(k, v)| k.len() + v.len())
        .sum();
    
    if estimated_size > MAX_CACHE_SIZE {
        return Err(cuenv_core::Error::configuration(format!(
            "Environment too large to cache: ~{estimated_size} bytes (max: {MAX_CACHE_SIZE} bytes)"
        )));
    }

    let cache_file = cache_dir.join(format!("{input_hash}.json"));

    let captured = CapturedEnvironment {
        env_vars,
        input_hash: input_hash.to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to get system time: {e}"))
            })?
            .as_secs(),
    };

    let content = serde_json::to_string_pretty(&captured)
        .map_err(|e| cuenv_core::Error::configuration(format!("Failed to serialize cache: {e}")))?;

    // Final check on actual serialized size
    if content.len() > MAX_CACHE_SIZE {
        return Err(cuenv_core::Error::configuration(format!(
            "Serialized environment too large: {} bytes (max: {MAX_CACHE_SIZE} bytes)",
            content.len()
        )));
    }

    fs::write(&cache_file, content)
        .map_err(|e| cuenv_core::Error::file_system(&cache_file, "write cache", e))?;

    // Also write to a "latest" file for the main process to read
    let latest_file = cache_dir.join("latest_env.json");
    fs::copy(&cache_file, &latest_file)
        .map_err(|e| cuenv_core::Error::file_system(&latest_file, "copy to latest", e))?;

    Ok(())
}

/// Apply cached environment to current process
pub fn apply_cached_environment(cache_dir: &Path, cached: CapturedEnvironment) -> Result<()> {
    eprintln!("# cuenv: Using cached environment (inputs unchanged)");

    // Write to latest file for main process to read
    let latest_file = cache_dir.join("latest_env.json");
    let content = serde_json::to_string_pretty(&cached)
        .map_err(|e| cuenv_core::Error::configuration(format!("Failed to serialize cache: {e}")))?;

    fs::write(&latest_file, content)
        .map_err(|e| cuenv_core::Error::file_system(&latest_file, "write latest", e))?;

    Ok(())
}

/// Calculate a hash of all hook inputs (commands, args, env vars, working dirs)
pub fn calculate_input_hash(hooks: &[Hook]) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    for hook in hooks {
        // Hash the hook command and args
        hasher.update(hook.command.as_bytes());
        if let Some(args) = &hook.args {
            for arg in args {
                hasher.update(arg.as_bytes());
            }
        }

        // Hash the working directory if set
        if let Some(dir) = &hook.dir {
            hasher.update(dir.as_bytes());
        }

        // Hash inputs if provided
        if let Some(inputs) = &hook.inputs {
            for input in inputs {
                hasher.update(input.as_bytes());
            }
        }

        // Hash inputs directory patterns if provided
        if let Some(inputs) = &hook.inputs {
            for pattern in inputs {
                // Check if this looks like a glob pattern
                if pattern.contains('*') || pattern.contains('?') {
                    // Use glob pattern to find matching files
                    let glob = Glob::new(pattern)
                        .map_err(|e| cuenv_core::Error::configuration(format!("Invalid glob pattern: {e}")))?
                        .compile_matcher();

                    // Walk the current directory and hash all matching paths
                    for entry in WalkDir::new(".").max_depth(5).into_iter().flatten() {
                        let path = entry.path();
                        if glob.is_match(path) {
                            hasher.update(path.to_string_lossy().as_bytes());
                            // Also hash the modification time if available
                            if let Ok(metadata) = entry.metadata() {
                                if let Ok(modified) = metadata.modified() {
                                    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                                        hasher.update(duration.as_secs().to_le_bytes());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Just hash the plain input
                    hasher.update(pattern.as_bytes());
                }
            }
        }

        // Add source flag to hash
        hasher.update([hook.source.unwrap_or(false) as u8]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}