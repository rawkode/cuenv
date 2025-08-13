use cuenv_config::Hook;
use cuenv_core::Result;
use cuenv_utils::directory_lock::DirectoryLock;
use cuenv_utils::hooks_status::{HookState, HooksStatusManager};
use globset::Glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::process::Command;
use tokio::time::timeout;
use walkdir::WalkDir;

use super::interactive::InteractiveHandler;

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

/// The mode in which the supervisor should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorMode {
    /// Run hooks interactively in the foreground, allowing user to background them.
    Foreground,
    /// Run hooks in the background without user interaction.
    Background,
    /// Run hooks synchronously, blocking until completion.
    Synchronous,
}

/// Preload hook supervisor that manages background hook execution
pub struct Supervisor {
    /// Hooks to execute
    hooks: Vec<Hook>,
    /// The mode in which the supervisor is running
    mode: SupervisorMode,
    /// Status manager for progress tracking
    status_manager: Arc<HooksStatusManager>,
    /// Handler for interactive terminal operations (optional)
    interactive_handler: Option<InteractiveHandler>,
    /// Directory lock (held for lifetime of supervisor)
    _lock: Option<DirectoryLock>,
    /// Directory for caching
    cache_dir: PathBuf,
}

impl Supervisor {
    /// Create a new preload supervisor
    pub fn new(hooks: Vec<Hook>, mode: SupervisorMode) -> Result<Self> {
        let status_manager = HooksStatusManager::new().map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {e}"))
        })?;

        let cache_dir = get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| cuenv_core::Error::file_system(&cache_dir, "create directory", e))?;

        let status_manager_arc = Arc::new(status_manager);

        let interactive_handler = if mode == SupervisorMode::Foreground {
            Some(InteractiveHandler::with_status_manager(Arc::clone(
                &status_manager_arc,
            )))
        } else {
            None
        };

        Ok(Self {
            hooks,
            mode,
            status_manager: status_manager_arc,
            interactive_handler,
            _lock: None, // Legacy mode doesn't use locking
            cache_dir,
        })
    }

    /// Create a new preload supervisor for a specific directory
    pub fn new_for_directory(
        directory: &Path,
        hooks: Vec<Hook>,
        mode: SupervisorMode,
    ) -> Result<Self> {
        // Try to acquire directory lock if in foreground mode
        let lock = if matches!(mode, SupervisorMode::Foreground) {
            match DirectoryLock::try_acquire(directory) {
                Ok(lock) => Some(lock),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Hooks already running for this directory
                    return Err(cuenv_core::Error::configuration(
                        "Hooks already running for this directory",
                    ));
                }
                Err(e) => {
                    return Err(cuenv_core::Error::file_system(
                        directory,
                        "acquire directory lock",
                        e,
                    ));
                }
            }
        } else {
            None
        };

        // Create directory-specific status manager
        let status_manager = HooksStatusManager::new_for_directory(directory).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to create status manager: {e}"))
        })?;

        let cache_dir = cuenv_utils::paths::get_state_dir(directory);
        fs::create_dir_all(&cache_dir)
            .map_err(|e| cuenv_core::Error::file_system(&cache_dir, "create directory", e))?;

        let status_manager_arc = Arc::new(status_manager);

        let interactive_handler = if mode == SupervisorMode::Foreground {
            Some(InteractiveHandler::with_status_manager(Arc::clone(
                &status_manager_arc,
            )))
        } else {
            None
        };

        Ok(Self {
            hooks,
            mode,
            status_manager: status_manager_arc,
            interactive_handler,
            _lock: lock,
            cache_dir,
        })
    }

    /// Run the supervisor
    pub async fn run(mut self) -> Result<()> {
        match self.mode {
            SupervisorMode::Foreground => self.run_foreground().await,
            SupervisorMode::Background => self.run_background().await,
            SupervisorMode::Synchronous => self.run_synchronous().await,
        }
    }

    async fn run_foreground(&mut self) -> Result<()> {
        if self.hooks.is_empty() {
            return Ok(());
        }

        // Check if hooks are already running
        let current_status = self.status_manager.get_current_status();

        // Check for stale hooks (marked as running but process is dead)
        let mut has_stale_hooks = false;
        let mut has_actually_running_hooks = false;

        for hook in current_status.hooks.values() {
            if matches!(hook.status, HookState::Running | HookState::Pending) {
                if let Some(pid) = hook.pid {
                    if is_process_running(pid) {
                        has_actually_running_hooks = true;
                    } else {
                        has_stale_hooks = true;
                    }
                } else if matches!(hook.status, HookState::Pending) {
                    // Pending hooks don't have PIDs yet
                    has_actually_running_hooks = true;
                }
            }
        }

        if has_stale_hooks {
            eprintln!(
                "# cuenv: Detected stale hooks (processes no longer running), clearing status..."
            );
            let _ = self.status_manager.clear_status();
        } else if has_actually_running_hooks {
            eprintln!("# cuenv: Hooks are already running, skipping...");
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = calculate_input_hash(&self.hooks)?;
        if let Ok(cached_env) = self.load_cached_environment(&input_hash) {
            // Inputs haven't changed, use cached environment
            eprintln!("# cuenv: Using cached environment (inputs unchanged)");
            self.apply_cached_environment(cached_env)?;
            return Ok(());
        }

        eprintln!("# cuenv: Running {} hook(s)...", self.hooks.len());

        // Clear any stale status from previous runs before initializing new hooks
        let _ = self.status_manager.clear_status();

        // Initialize status tracking with all hooks
        let hook_names: Vec<String> = self
            .hooks
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
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {e}"))
            })?;

        let mut handles = Vec::new();

        for hook in self.hooks.iter().cloned() {
            let status_manager = self.status_manager.clone();
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let handle = tokio::spawn(async move {
                // Start with no PID, will be updated when process spawns
                let _ = status_manager.mark_hook_started(&hook_key, 0);
                let result = execute_hook_with_timeout(&hook, Duration::from_secs(60)).await;
                match result {
                    Ok((output, pid)) => {
                        // Update with actual PID if we got one
                        if let Some(actual_pid) = pid {
                            let _ = status_manager.mark_hook_started(&hook_key, actual_pid);
                        }
                        let _ = status_manager.mark_hook_completed(&hook_key);
                        output
                    }
                    Err(e) => {
                        let _ = status_manager.mark_hook_failed(&hook_key, e.to_string());
                        None
                    }
                }
            });
            handles.push(handle);
        }

        let mut message_shown = false;
        let start_time = std::time::Instant::now();

        loop {
            let all_finished = handles.iter().all(|h| h.is_finished());
            if all_finished {
                break;
            }

            // Show message after 1 second
            if !message_shown && start_time.elapsed() > Duration::from_secs(1) {
                eprintln!("# cuenv: Press 'b' to background, 'q' to quit");
                message_shown = true;
            }

            // Always check for input after message is shown
            if message_shown {
                if let Some(interactive_handler) = &mut self.interactive_handler {
                    if interactive_handler
                        .monitor_with_timeout(Duration::from_millis(200))
                        .await
                        == super::interactive::ControlFlow::Background
                    {
                        // Spawn a background task to monitor hook completion
                        eprintln!(
                            "# cuenv: Continuing {} hook(s) in background...",
                            handles.len()
                        );

                        let _status_manager = self.status_manager.clone();
                        let hooks = self.hooks.clone();
                        let cache_dir = self.cache_dir.clone();

                        tokio::spawn(async move {
                            // Wait for all handles to complete
                            let mut captured_env = HashMap::new();
                            for handle in handles {
                                if let Ok(Some(env)) = handle.await {
                                    captured_env.extend(env);
                                }
                            }

                            // Save captured environment if any
                            if !captured_env.is_empty() {
                                if let Ok(input_hash) = calculate_input_hash(&hooks) {
                                    let captured_env_obj = CapturedEnvironment {
                                        env_vars: captured_env,
                                        input_hash: input_hash.clone(),
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs(),
                                    };

                                    if let Ok(captured) = serde_json::to_string(&captured_env_obj) {
                                        let cache_file =
                                            cache_dir.join(format!("{input_hash}.json"));
                                        let _ = fs::write(&cache_file, &captured);

                                        // Also write to latest_env.json for shell to read
                                        let latest_file = cache_dir.join("latest_env.json");
                                        let _ = fs::write(&latest_file, &captured);
                                    }
                                }
                            }

                            // DON'T clear status - keep it available for status command
                            // The status will be cleared on next directory change
                        });

                        return Ok(());
                    }
                }
            } else {
                // Before message is shown, just sleep briefly
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let mut captured_env = HashMap::new();
        for handle in handles {
            if let Ok(Some(env)) = handle.await {
                captured_env.extend(env);
            }
        }

        if !captured_env.is_empty() {
            let input_hash = calculate_input_hash(&self.hooks)?;
            self.save_cached_environment(&input_hash, captured_env)?;
        }

        self.status_manager.clear_status()?;
        eprintln!("# cuenv: ✓ All hooks completed");
        Ok(())
    }

    async fn run_synchronous(&self) -> Result<()> {
        // For now, synchronous is the same as background
        self.execute_hooks_in_background().await
    }

    async fn run_background(&self) -> Result<()> {
        self.execute_hooks_in_background().await
    }

    async fn execute_hooks_in_background(&self) -> Result<()> {
        if self.hooks.is_empty() {
            return Ok(());
        }

        // Check if we need to run based on inputs
        let input_hash = calculate_input_hash(&self.hooks)?;
        if let Ok(cached_env) = self.load_cached_environment(&input_hash) {
            // Inputs haven't changed, use cached environment
            self.apply_cached_environment(cached_env)?;
            return Ok(());
        }

        eprintln!(
            "# cuenv: Starting {} hook(s) in background...",
            self.hooks.len()
        );

        // Clear any stale status from previous runs
        let _ = self.status_manager.clear_status();

        // Initialize status tracking
        let hook_names: Vec<String> = self
            .hooks
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
                cuenv_core::Error::configuration(format!("Failed to initialize hooks: {e}"))
            })?;

        // Collect environment from source hooks
        let mut captured_env = HashMap::new();

        // Execute all hooks
        let mut handles = Vec::new();
        for hook in self.hooks.iter().cloned() {
            let hook_key = if let Some(args) = &hook.args {
                format!("{} {:?}", hook.command, args)
            } else {
                hook.command.clone()
            };

            let status_manager = self.status_manager.clone();
            let hook_clone = hook.clone();
            let hook_key_clone = hook_key.clone();

            eprintln!("# cuenv: Running hook: {}", hook.command);

            // Mark hook as started
            let pid = std::process::id();
            status_manager
                .mark_hook_started(&hook_key, pid)
                .map_err(|e| {
                    cuenv_core::Error::configuration(format!("Failed to update status: {e}"))
                })?;

            let handle = tokio::spawn(async move {
                let result = execute_hook_with_timeout(&hook_clone, Duration::from_secs(60)).await;

                match result {
                    Ok((output, pid)) => {
                        // Update with actual PID if we got one
                        if let Some(actual_pid) = pid {
                            let _ = status_manager.mark_hook_started(&hook_key_clone, actual_pid);
                        }
                        eprintln!("# cuenv: Hook completed: {}", hook_clone.command);
                        let _ = status_manager.mark_hook_completed(&hook_key_clone);

                        // Return environment if this was a source hook
                        output
                    }
                    Err(e) => {
                        eprintln!("# cuenv: Hook failed: {}: {}", hook_clone.command, e);
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
            cuenv_core::Error::configuration(format!("Failed to clear status: {e}"))
        })?;

        eprintln!("# cuenv: All hooks completed");
        Ok(())
    }

    /// Load cached environment for given input hash
    fn load_cached_environment(&self, input_hash: &str) -> Result<CapturedEnvironment> {
        let cache_file = self.cache_dir.join(format!("{input_hash}.json"));

        if !cache_file.exists() {
            return Err(cuenv_core::Error::configuration("Cache file not found"));
        }

        let content = fs::read_to_string(&cache_file)
            .map_err(|e| cuenv_core::Error::file_system(&cache_file, "read cache", e))?;

        serde_json::from_str(&content)
            .map_err(|e| cuenv_core::Error::configuration(format!("Failed to parse cache: {e}")))
    }

    /// Save captured environment to cache
    fn save_cached_environment(
        &self,
        input_hash: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<()> {
        let cache_file = self.cache_dir.join(format!("{input_hash}.json"));

        let captured = CapturedEnvironment {
            env_vars,
            input_hash: input_hash.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let content = serde_json::to_string_pretty(&captured).map_err(|e| {
            cuenv_core::Error::configuration(format!("Failed to serialize cache: {e}"))
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
            cuenv_core::Error::configuration(format!("Failed to serialize cache: {e}"))
        })?;

        fs::write(&latest_file, content)
            .map_err(|e| cuenv_core::Error::file_system(&latest_file, "write latest", e))?;

        Ok(())
    }
}

/// Execute a hook with timeout and capture environment if needed
/// Returns the output and the actual process PID
async fn execute_hook_with_timeout(
    hook: &Hook,
    timeout_duration: Duration,
) -> Result<(Option<HashMap<String, String>>, Option<u32>)> {
    // For source hooks, we need to evaluate the output as shell script
    if hook.source.unwrap_or(false) {
        // Create a wrapper script that evaluates the hook output and captures env changes
        let wrapper_script = format!(
            r#"
# Save current environment to a temp file with null separation
TEMP_BEFORE=$(mktemp)
TEMP_AFTER=$(mktemp)
trap "rm -f $TEMP_BEFORE $TEMP_AFTER" EXIT

# Save environment with null bytes as separators (handles newlines in values)
env -0 | sort -z > "$TEMP_BEFORE"

# Run the hook command and capture its output
HOOK_OUTPUT="$({} {})"

# Evaluate the output as shell script (this is what direnv does)
eval "$HOOK_OUTPUT"

# Get the new environment with null separation
env -0 | sort -z > "$TEMP_AFTER"

# Output only new/changed variables with null separation for proper parsing
comm -z -13 "$TEMP_BEFORE" "$TEMP_AFTER"
"#,
            hook.command,
            hook.args
                .as_ref()
                .map(|args| args
                    .iter()
                    .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                    .collect::<Vec<_>>()
                    .join(" "))
                .unwrap_or_default()
        );

        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&wrapper_script);

        if let Some(dir) = &hook.dir {
            cmd.current_dir(dir);
        }

        // Capture output
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null());

        // Spawn the process to get its PID
        let child = cmd.spawn().map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to spawn hook: {e}"),
                None,
            )
        })?;

        // Get the actual child process PID
        let pid = child.id();

        // Wait for completion with timeout
        let output = match timeout(timeout_duration, child.wait_with_output()).await {
            Ok(result) => result,
            Err(_) => {
                return Err(cuenv_core::Error::command_execution(
                    hook.command.clone(),
                    hook.args.clone().unwrap_or_default(),
                    "Hook execution timed out".to_string(),
                    None,
                ));
            }
        }
        .map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute hook: {e}"),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Hook failed: {stderr}"),
                output.status.code(),
            ));
        }

        // Parse null-separated output to handle multiline values correctly
        let mut env_vars = HashMap::new();
        let stdout_bytes = &output.stdout;

        // Split on null bytes to get each environment variable
        for entry in stdout_bytes.split(|&b| b == 0) {
            if entry.is_empty() {
                continue;
            }

            // Convert to string
            let entry_str = String::from_utf8_lossy(entry);

            // Find the first '=' to split key and value
            if let Some(eq_pos) = entry_str.find('=') {
                let key = &entry_str[..eq_pos];
                let value = &entry_str[eq_pos + 1..];

                // Validate key is a valid environment variable name
                // Exclude special shell variables like '_' which is read-only in bash
                if !key.is_empty()
                    && key != "_"  // Exclude the read-only _ variable
                    && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !key.chars().next().unwrap().is_ascii_digit()
                {
                    env_vars.insert(key.to_string(), value.to_string());
                }
            }
        }

        Ok((Some(env_vars), pid))
    } else {
        // Non-source hooks run normally
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

        // Spawn the process to get its PID
        let child = cmd.spawn().map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to spawn hook: {e}"),
                None,
            )
        })?;

        // Get the actual child process PID
        let pid = child.id();

        // Wait for completion with timeout
        let output = match timeout(timeout_duration, child.wait_with_output()).await {
            Ok(result) => result,
            Err(_) => {
                return Err(cuenv_core::Error::command_execution(
                    hook.command.clone(),
                    hook.args.clone().unwrap_or_default(),
                    "Hook execution timed out".to_string(),
                    None,
                ));
            }
        }
        .map_err(|e| {
            cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Failed to execute hook: {e}"),
                None,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(cuenv_core::Error::command_execution(
                hook.command.clone(),
                hook.args.clone().unwrap_or_default(),
                format!("Hook failed: {stderr}"),
                output.status.code(),
            ));
        }

        Ok((None, pid))
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
                    for entry in WalkDir::new(".").into_iter().flatten() {
                        let path = entry.path();
                        if matcher.is_match(path) {
                            matched_files.push(path.to_path_buf());
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

/// Check if a process with the given PID is running
fn is_process_running(pid: u32) -> bool {
    // Use /proc filesystem on Unix-like systems
    #[cfg(unix)]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }

    #[cfg(not(unix))]
    {
        // For non-Unix systems, conservatively assume it's not running
        // This will clear stale status on Windows
        false
    }
}

/// Get the cache directory for preload hooks
pub fn get_cache_dir() -> Result<PathBuf> {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string());

    let cache_dir = PathBuf::from(format!("/tmp/cuenv-{user}/preload-cache"));
    Ok(cache_dir)
}

/// Entry point for the supervisor process
pub async fn run_supervisor(hooks: Vec<Hook>) -> Result<()> {
    // For now, we'll default to background mode. This will be updated later.
    let supervisor = Supervisor::new(hooks, SupervisorMode::Background)?;
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
        let (env, _pid) = result.unwrap();
        assert_eq!(env, None, "Non-source hook should return None");
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

        let supervisor = Supervisor::new(hooks, SupervisorMode::Background).unwrap();
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
