use crate::config::{DevenvConfig, ExecConfig, NixFlakeConfig};
use crate::core::errors::Result;
use crate::env::{
    evaluate_shell_environment, filter_environment, merge_xdg_data_dirs, parse_shell_exports,
    EnvCache,
};
use anyhow::Context;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{debug, info, warn};
use which::which;

/// Execute a nix flake hook and return the environment
pub async fn execute_nix_flake_hook(
    flake: &NixFlakeConfig,
    cache: &EnvCache,
    force_reload: bool,
) -> Result<HashMap<String, String>> {
    debug!("execute_nix_flake_hook called with flake: {:?}", flake);
    // Check cache first unless forced to reload
    if !force_reload {
        if let Ok(cached_env) = cache.load() {
            // Check if cache is still valid using file watcher
            let watch_files =
                crate::env::default_watch_files(Path::new(flake.dir.as_deref().unwrap_or(".")));
            let watcher = crate::env::FileWatcher::new(watch_files);

            if watcher.cache_is_valid(&cache.cache_file()) {
                info!("Using cached nix environment");
                return Ok(cached_env);
            }
        }
    }

    debug!("Building nix development environment...");
    info!("Building nix development environment...");

    // Build nix command
    let nix_cmd = build_nix_develop_command(flake)?;

    // Execute and capture output
    debug!("About to execute nix command");
    let output = execute_command(nix_cmd)?;
    debug!("Nix command executed, got {} bytes of output", output.len());

    // Save raw RC output to cache
    cache
        .save_rc(&output)
        .context("Failed to save nix output to cache")?;

    // Evaluate the shell script to get the actual environment
    let mut env =
        evaluate_shell_environment(&output).context("Failed to evaluate nix environment")?;

    // Filter out unwanted variables
    env = filter_environment(env);

    // Special handling for XDG_DATA_DIRS
    if let Ok(current_xdg) = std::env::var("XDG_DATA_DIRS") {
        if let Some(new_xdg) = env.get("XDG_DATA_DIRS") {
            if let Some(merged) = merge_xdg_data_dirs(Some(current_xdg), Some(new_xdg.clone())) {
                env.insert("XDG_DATA_DIRS".to_string(), merged);
            }
        }
    }

    // Save parsed environment to cache
    cache
        .save(&env)
        .context("Failed to save environment to cache")?;

    info!("Nix environment loaded with {} variables", env.len());
    debug!("Nix PATH = {:?}", env.get("PATH"));
    Ok(env)
}

/// Execute a devenv hook and return the environment
pub async fn execute_devenv_hook(
    devenv: &DevenvConfig,
    cache: &EnvCache,
    force_reload: bool,
) -> Result<HashMap<String, String>> {
    // Check cache first unless forced to reload
    if !force_reload {
        if let Ok(cached_env) = cache.load() {
            let watch_files =
                crate::env::default_watch_files(Path::new(devenv.dir.as_deref().unwrap_or(".")));
            let watcher = crate::env::FileWatcher::new(watch_files);

            if watcher.cache_is_valid(&cache.cache_file()) {
                info!("Using cached devenv environment");
                return Ok(cached_env);
            }
        }
    }

    info!("Building devenv environment...");

    // Build devenv command
    let devenv_cmd = build_devenv_command(devenv)?;

    // Execute and capture output
    let output = execute_command(devenv_cmd)?;

    // Save raw RC output to cache
    cache
        .save_rc(&output)
        .context("Failed to save devenv output to cache")?;

    // Evaluate the shell script to get the actual environment
    let mut env =
        evaluate_shell_environment(&output).context("Failed to evaluate devenv environment")?;

    // Filter out unwanted variables
    env = filter_environment(env);

    // Save parsed environment to cache
    cache
        .save(&env)
        .context("Failed to save environment to cache")?;

    info!("Devenv environment loaded with {} variables", env.len());
    Ok(env)
}

/// Execute a generic hook with source support
pub async fn execute_source_hook(
    exec: &ExecConfig,
    cache: Option<&EnvCache>,
) -> Result<HashMap<String, String>> {
    if !exec.source.unwrap_or(false) {
        // Just execute without capturing
        let mut cmd = Command::new(&exec.command);
        if let Some(args) = &exec.args {
            cmd.args(args);
        }
        if let Some(dir) = &exec.dir {
            cmd.current_dir(dir);
        }

        let status = cmd.status().context("Failed to execute hook command")?;

        if !status.success() {
            warn!("Hook command failed with status: {:?}", status.code());
        }

        return Ok(HashMap::new());
    }

    // Check cache if available
    if let Some(cache) = cache {
        if let Ok(cached_env) = cache.load() {
            debug!("Using cached environment for source hook");
            return Ok(cached_env);
        }
    }

    info!("Executing source hook: {}", exec.command);

    let mut cmd = Command::new(&exec.command);
    if let Some(args) = &exec.args {
        cmd.args(args);
    }
    if let Some(dir) = &exec.dir {
        cmd.current_dir(dir);
    }

    let output = execute_command(cmd)?;

    // Parse the environment
    let env = parse_shell_exports(&output).context("Failed to parse hook environment")?;

    // Save to cache if available
    if let Some(cache) = cache {
        cache
            .save(&env)
            .context("Failed to save environment to cache")?;
    }

    Ok(env)
}

/// Build the nix develop command
fn build_nix_develop_command(flake: &NixFlakeConfig) -> Result<Command> {
    // Check if nix is available
    let nix_path = which("nix").context("nix command not found. Please install Nix")?;

    let mut cmd = Command::new(nix_path);

    // Add standard nix flags
    cmd.args(&[
        "--extra-experimental-features",
        "nix-command flakes",
        "print-dev-env",
    ]);

    // Add impure flag if requested
    if flake.impure.unwrap_or(false) {
        cmd.arg("--impure");
    }

    // Build flake reference
    let flake_ref = if let Some(reference) = &flake.reference {
        reference.clone()
    } else if let Some(dir) = &flake.dir {
        format!("path:{}", dir)
    } else {
        ".".to_string()
    };

    // Add shell selector if specified
    let full_ref = if let Some(shell) = &flake.shell {
        format!("{}#{}", flake_ref, shell)
    } else {
        flake_ref
    };

    cmd.arg(full_ref);

    debug!("Nix command: {:?}", cmd);
    Ok(cmd)
}

/// Build the devenv command
fn build_devenv_command(devenv: &DevenvConfig) -> Result<Command> {
    // Check if devenv is available
    let devenv_path = which("devenv").context("devenv command not found. Please install devenv")?;

    let mut cmd = Command::new(devenv_path);

    cmd.arg("print-dev-env");

    // Add directory if specified
    if let Some(dir) = &devenv.dir {
        cmd.current_dir(dir);
    }

    // Add profile if specified
    if let Some(profile) = &devenv.profile {
        cmd.args(&["--profile", profile]);
    }

    // Add additional options
    if let Some(options) = &devenv.options {
        for opt in options {
            cmd.arg(opt);
        }
    }

    debug!("Devenv command: {:?}", cmd);
    Ok(cmd)
}

/// Execute a command and capture its output
fn execute_command(mut cmd: Command) -> Result<String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let output = cmd.output().context("Failed to execute command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            warn!("Command stderr: {}", stderr);
        }

        // For nix/devenv, we might still have useful output even on non-zero exit
        // (e.g., warnings that don't prevent environment generation)
        if output.stdout.is_empty() {
            return Err(crate::core::errors::Error::configuration(format!(
                "Command failed with status {:?}: {}",
                output.status.code(),
                stderr
            )));
        }
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_nix_command_simple() {
        let flake = NixFlakeConfig {
            dir: Some(".".to_string()),
            reference: None,
            shell: None,
            impure: None,
        };

        let cmd = build_nix_develop_command(&flake);
        // Command should be built (will fail if nix not installed, which is OK for test)
        assert!(
            cmd.is_ok()
                || cmd
                    .unwrap_err()
                    .to_string()
                    .contains("nix command not found")
        );
    }

    #[test]
    fn test_build_nix_command_with_shell() {
        let flake = NixFlakeConfig {
            dir: None,
            reference: Some("github:owner/repo".to_string()),
            shell: Some("devShell".to_string()),
            impure: Some(true),
        };

        let cmd = build_nix_develop_command(&flake);
        assert!(
            cmd.is_ok()
                || cmd
                    .unwrap_err()
                    .to_string()
                    .contains("nix command not found")
        );
    }
}
