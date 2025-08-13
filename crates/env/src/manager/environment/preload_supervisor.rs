use cuenv_config::Hook;
use cuenv_core::Result;
use cuenv_utils::hooks_status::HooksStatusManager;
use globset::Glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::process::Command;
use tokio::time::timeout;
use walkdir::WalkDir;

/// Default timeout for preload hooks (60 seconds)
const DEFAULT_PRELOAD_TIMEOUT: Duration = Duration::from_secs(60);

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

/// Preload hook supervisor that manages background hook execution
pub struct PreloadSupervisor {
    /// Hooks to execute
    hooks: Vec<Hook>,
    /// Status manager for progress tracking
    status_manager: Arc<HooksStatusManager>,
    /// Directory for caching
    cache_dir: PathBuf,
    /// Timeout for hooks
    timeout: Duration,
}

impl PreloadSupervisor {
    /// Create a new preload supervisor
    pub fn new(hooks: Vec<Hook>) -> Result<Self> {
        let status_manager = HooksStatusManager::new().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {}", e))
        })?;

        let cache_dir = get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| cuenv_core::Error::file_system(&cache_dir, "create directory", e))?;

        Ok(Self {
            hooks,
            status_manager: Arc::new(status_manager),
            cache_dir,
            timeout: DEFAULT_PRELOAD_TIMEOUT,
        })
    }

    /// Run the supervisor
    pub async fn run(self) -> Result<()> {
        // Filter only preload hooks
        let preload_hooks: Vec<Hook> = self
            .hooks
            .iter()
            .filter(|h| h.preload.unwrap_or(false))
            .cloned()
            .collect();

        if preload_hooks.is_empty() {
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = calculate_input_hash(&preload_hooks)?;
        if let Ok(cached_env) = self.load_cached_environment(&input_hash) {
            // Inputs haven't changed, use cached environment
            self.apply_cached_environment(cached_env)?;
            return Ok(());
        }

        eprintln!(
            "# cuenv: Starting {} preload hook(s) in background...",
            preload_hooks.len()
        );

        // Initialize status tracking
        let hook_names: Vec<String> = preload_hooks
            .iter()
            .map(|h| {
                if let Some(args) = &h.args {
                    format!("{} {:?}", h.command, args)
                } else {
                    h.command.clone()
                }
            })
            .collect();

        self.status_manager
            .initialize_hooks(hook_names)
            .map_err(|e| {
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {}", e))
            })?;

        // Collect environment from source hooks
        let mut captured_env = HashMap::new();

        // Execute all hooks
        let mut handles = Vec::new();
        for hook in preload_hooks {
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let status_manager = self.status_manager.clone();
            let hook_clone = hook.clone();
            let hook_key_clone = hook_key.clone();

            eprintln!("# cuenv: Running preload hook: {}", hook.command);

            // Mark hook as started
            let pid = std::process::id();
            status_manager
                .mark_hook_started(&hook_key, pid)
                .map_err(|e| {
                    cuenv_core::Error::configuration(format!("Failed to update status: {}", e))
                })?;

            let handle = tokio::spawn(async move {
                let result = execute_hook_with_timeout(&hook_clone, Duration::from_secs(60)).await;

                match result {
                    Ok(output) => {
                        eprintln!("# cuenv: Preload hook completed: {}", hook_clone.command);
                        let _ = status_manager.mark_hook_completed(&hook_key_clone);

                        // Return environment if this was a source hook
                        if hook_clone.source.unwrap_or(false) {
                            output
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "# cuenv: Preload hook failed: {}: {}",
                            hook_clone.command, e
                        );
                        let _ = status_manager.mark_hook_failed(&hook_key_clone, e.to_string());
                        None
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all hooks to complete
        for handle in handles {
            if let Ok(Some(env)) = handle.await {
                // Merge captured environment
                captured_env.extend(env);
            }
        }

        // Save captured environment to cache
        if !captured_env.is_empty() {
            self.save_cached_environment(&input_hash, captured_env)?;
        }

        // Clean up status
        self.status_manager.clear_status().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to clear status: {}", e))
        })?;

        eprintln!("# cuenv: All preload hooks completed");
        Ok(())
    }

    /// Load cached environment for given input hash
    fn load_cached_environment(&self, input_hash: &str) -> Result<CapturedEnvironment> {
        let cache_file = self.cache_dir.join(format!("{}.json", input_hash));

        if !cache_file.exists() {
            return Err(cuenv_core::Error::configuration("Cache file not found"));
        }

        let content = fs::read_to_string(&cache_file)
            .map_err(|e| cuenv_core::Error::file_system(&cache_file, "read cache", e))?;

        serde_json::from_str(&content)
            .map_err(|e| cuenv_core::Error::configuration(format!("Failed to parse cache: {}", e)))
    }

    /// Save captured environment to cache
    fn save_cached_environment(
        &self,
        input_hash: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<()> {
        let cache_file = self.cache_dir.join(format!("{}.json", input_hash));

        let captured = CapturedEnvironment {
            env_vars,
            input_hash: input_hash.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let content = serde_json::to_string_pretty(&captured).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to serialize cache: {}", e))
        })?;

        fs::write(&cache_file, content)
            .map_err(|e| cuenv_core::Error::file_system(&cache_file, "write cache", e))?;

        // Also write to a "latest" file for the main process to read
        let latest_file = self.cache_dir.join("latest_env.json");
        fs::copy(&cache_file, &latest_file)
            .map_err(|e| cuenv_core::Error::file_system(&latest_file, "copy to latest", e))?;

        Ok(())
    }

    /// Apply cached environment to current process
    fn apply_cached_environment(&self, cached: CapturedEnvironment) -> Result<()> {
        eprintln!("# cuenv: Using cached environment (inputs unchanged)");

        // Write to latest file for main process to read
        let latest_file = self.cache_dir.join("latest_env.json");
        let content = serde_json::to_string_pretty(&cached).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to serialize cache: {}", e))
        })?;

        fs::write(&latest_file, content)
            .map_err(|e| cuenv_core::Error::file_system(&latest_file, "write latest", e))?;

        Ok(())
    }
}

/// Execute a hook with timeout and capture environment if needed
async fn execute_hook_with_timeout(
    hook: &Hook,
    timeout_duration: Duration,
) -> Result<Option<HashMap<String, String>>> {
    let mut cmd = Command::new(&hook.command);

    if let Some(args) = &hook.args {
        cmd.args(args);
    }

    if let Some(dir) = &hook.dir {
        cmd.current_dir(dir);
    }

    // Capture output
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());

    let output = timeout(timeout_duration, cmd.output())
        .await
        .map_err(|_| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                "Hook execution timed out".to_string(),
                None,
            )
        })?
        .map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute hook: {}", e),
                None,
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(cuenv_core::Error::command_execution(
            hook.command.clone(),
            hook.args.clone().unwrap_or_default(),
            format!("Hook failed: {}", stderr),
            output.status.code(),
        ));
    }

    // If this is a source hook, parse environment
    if hook.source.unwrap_or(false) {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse environment variables from output
        // This is a simplified parser - you may need to adapt based on actual output format
        let mut env_vars = HashMap::new();
        for line in stdout.lines() {
            if let Some((key, value)) = parse_env_line(line) {
                env_vars.insert(key, value);
            }
        }

        Ok(Some(env_vars))
    } else {
        Ok(None)
    }
}

/// Parse an environment variable line (e.g., "export FOO=bar" or "FOO=bar")
fn parse_env_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();

    // Skip comments and empty lines
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Handle "export KEY=VALUE" format
    let line = if line.starts_with("export ") {
        &line[7..]
    } else {
        line
    };

    // Parse KEY=VALUE
    if let Some(eq_pos) = line.find('=') {
        let key = line[..eq_pos].trim();
        let value = line[eq_pos + 1..].trim();

        // Remove quotes if present
        let value = if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            &value[1..value.len() - 1]
        } else {
            value
        };

        Some((key.to_string(), value.to_string()))
    } else {
        None
    }
}

/// Calculate hash of hook inputs to determine if re-execution is needed
fn calculate_input_hash(hooks: &[Hook]) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    for hook in hooks {
        hasher.update(&hook.command);

        if let Some(args) = &hook.args {
            for arg in args {
                hasher.update(arg);
            }
        }

        if let Some(dir) = &hook.dir {
            hasher.update(dir);
        }

        // Hash the inputs if specified
        if let Some(inputs) = &hook.inputs {
            for input_pattern in inputs {
                // Use walkdir to find matching files
                let mut matched_files = Vec::new();

                // Build glob matcher
                if let Ok(glob) = Glob::new(input_pattern) {
                    let matcher = glob.compile_matcher();

                    // Walk current directory looking for matches
                    for entry in WalkDir::new(".") {
                        if let Ok(entry) = entry {
                            let path = entry.path();
                            if matcher.is_match(path) {
                                matched_files.push(path.to_path_buf());
                            }
                        }
                    }
                }

                // Sort files for consistent hashing
                matched_files.sort();

                for file in matched_files {
                    if let Ok(metadata) = fs::metadata(&file) {
                        hasher.update(file.to_string_lossy().as_bytes());
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                                hasher.update(duration.as_secs().to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Get the cache directory for preload hooks
fn get_cache_dir() -> Result<PathBuf> {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());

    let cache_dir = PathBuf::from(format!("/tmp/cuenv-{}/preload-cache", user));
    Ok(cache_dir)
}

/// Entry point for the supervisor process
pub async fn run_supervisor(hooks: Vec<Hook>) -> Result<()> {
    let supervisor = PreloadSupervisor::new(hooks)?;
    supervisor.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_hook(command: &str, args: Vec<String>, preload: bool, source: bool) -> Hook {
        Hook {
            command: command.to_string(),
            args: Some(args),
            dir: None,
            preload: Some(preload),
            source: Some(source),
            inputs: None,
        }
    }

    fn create_test_hook_with_inputs(command: &str, args: Vec<String>, inputs: Vec<String>) -> Hook {
        Hook {
            command: command.to_string(),
            args: Some(args),
            dir: None,
            preload: Some(true),
            source: Some(false),
            inputs: Some(inputs),
        }
    }

    #[test]
    fn test_parse_env_line() {
        // Test basic KEY=VALUE
        assert_eq!(
            parse_env_line("FOO=bar"),
            Some(("FOO".to_string(), "bar".to_string()))
        );

        // Test export KEY=VALUE
        assert_eq!(
            parse_env_line("export FOO=bar"),
            Some(("FOO".to_string(), "bar".to_string()))
        );

        // Test quoted values
        assert_eq!(
            parse_env_line("FOO=\"bar baz\""),
            Some(("FOO".to_string(), "bar baz".to_string()))
        );

        assert_eq!(
            parse_env_line("FOO='bar baz'"),
            Some(("FOO".to_string(), "bar baz".to_string()))
        );

        // Test comments and empty lines
        assert_eq!(parse_env_line("# comment"), None);
        assert_eq!(parse_env_line(""), None);
        assert_eq!(parse_env_line("   "), None);

        // Test whitespace handling
        assert_eq!(
            parse_env_line("  export FOO=bar  "),
            Some(("FOO".to_string(), "bar".to_string()))
        );
    }

    #[tokio::test]
    async fn test_calculate_input_hash_consistency() {
        let hooks = vec![
            create_test_hook("echo", vec!["hello".to_string()], true, false),
            create_test_hook("echo", vec!["world".to_string()], true, false),
        ];

        let hash1 = calculate_input_hash(&hooks).unwrap();
        let hash2 = calculate_input_hash(&hooks).unwrap();

        assert_eq!(hash1, hash2, "Hash should be consistent for same inputs");
    }

    #[tokio::test]
    async fn test_calculate_input_hash_changes_with_inputs() {
        let hooks1 = vec![create_test_hook(
            "echo",
            vec!["hello".to_string()],
            true,
            false,
        )];
        let hooks2 = vec![create_test_hook(
            "echo",
            vec!["world".to_string()],
            true,
            false,
        )];

        let hash1 = calculate_input_hash(&hooks1).unwrap();
        let hash2 = calculate_input_hash(&hooks2).unwrap();

        assert_ne!(hash1, hash2, "Hash should change with different inputs");
    }

    #[tokio::test]
    async fn test_execute_hook_with_timeout_success() {
        let hook = create_test_hook("echo", vec!["hello".to_string()], true, false);
        let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

        assert!(result.is_ok(), "Echo command should succeed");
        assert_eq!(result.unwrap(), None, "Non-source hook should return None");
    }

    #[tokio::test]
    async fn test_execute_hook_with_timeout_failure() {
        let hook = create_test_hook("false", vec![], true, false);
        let result = execute_hook_with_timeout(&hook, Duration::from_secs(5)).await;

        assert!(result.is_err(), "False command should fail");
    }

    #[tokio::test]
    async fn test_supervisor_no_preload_hooks() {
        let hooks = vec![
            create_test_hook("echo", vec!["test".to_string()], false, false), // Not a preload hook
        ];

        let supervisor = PreloadSupervisor::new(hooks).unwrap();
        let result = supervisor.run().await;

        assert!(
            result.is_ok(),
            "Supervisor should succeed with no preload hooks"
        );
    }

    #[tokio::test]
    async fn test_cache_directory_creation() {
        let cache_dir = get_cache_dir().unwrap();
        assert!(cache_dir.to_string_lossy().contains("cuenv"));
        assert!(cache_dir.to_string_lossy().contains("preload-cache"));
    }
}
