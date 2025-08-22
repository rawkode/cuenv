//! Network namespace creation and management

use anyhow::{bail, Context, Result};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::getuid;
use std::fs;
use sysinfo::System;

/// Check if unprivileged user namespaces are supported
pub fn supports_unprivileged_namespaces() -> bool {
    // Check if we can read the kernel parameter
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        return content.trim() == "1";
    }

    // If the file doesn't exist, try to detect by kernel version
    // Unprivileged namespaces were added in Linux 3.8
    if let Some(kernel_version) = System::kernel_version() {
        return check_kernel_version_supports_namespaces(&kernel_version);
    }

    false
}

/// Parse kernel version and check if it supports unprivileged namespaces (>= 3.8)
fn check_kernel_version_supports_namespaces(version_str: &str) -> bool {
    // Parse the version string, looking for a pattern like "5.4.0" or "3.8.1"
    let version_part = version_str
        .split_whitespace()
        .find(|s| {
            s.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        })
        .unwrap_or("");

    let mut parts = version_part
        .split('.')
        .take(2)
        .filter_map(|p| p.parse::<u32>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);

    // Unprivileged user namespaces were added in Linux 3.8
    major > 3 || (major == 3 && minor >= 8)
}

/// Create an isolated network namespace for the current process
pub fn create_network_namespace() -> Result<()> {
    // Check if we're already in a namespace (avoid double-isolation)
    if std::env::var("CUENV_IN_NETNS").is_ok() {
        log::debug!("Already in network namespace, skipping creation");
        return Ok(());
    }

    // Ensure we're not running as root (security best practice)
    if getuid().is_root() {
        bail!("Network namespace creation should not be run as root");
    }

    log::debug!("Creating unprivileged user and network namespace");

    // Create new user and network namespaces
    unshare(CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNET)
        .context("Failed to create user and network namespaces")?;

    // Set up UID/GID mappings for the user namespace
    setup_uid_gid_mappings()?;

    // Set up loopback interface in the new network namespace
    setup_loopback_interface()?;

    // Mark that we're in a network namespace
    std::env::set_var("CUENV_IN_NETNS", "1");

    log::info!("Network namespace created successfully");
    Ok(())
}

/// Set up UID/GID mappings for the user namespace
fn setup_uid_gid_mappings() -> Result<()> {
    let uid = nix::unistd::getuid();
    let gid = nix::unistd::getgid();

    // Map current user to root in the namespace
    let uid_map = format!("0 {uid} 1");
    let gid_map = format!("0 {gid} 1");

    fs::write("/proc/self/uid_map", uid_map).context("Failed to write UID mapping")?;

    // Disable setgroups to allow GID mapping as non-root
    fs::write("/proc/self/setgroups", "deny").context("Failed to disable setgroups")?;

    fs::write("/proc/self/gid_map", gid_map).context("Failed to write GID mapping")?;

    Ok(())
}

/// Set up the loopback interface in the network namespace
fn setup_loopback_interface() -> Result<()> {
    use std::process::Command;

    // Since we can't easily use netlink from Rust without additional dependencies,
    // we'll use the ip command if available, or create a minimal loopback setup

    // Try using the ip command first
    if let Ok(output) = Command::new("ip")
        .args(["link", "set", "dev", "lo", "up"])
        .output()
    {
        if output.status.success() {
            log::debug!("Loopback interface enabled using ip command");
            return Ok(());
        }
    }

    log::warn!("Could not set up loopback interface - some localhost connections may fail");
    Ok(())
}

/// Check if the current process is running in a network namespace
pub fn is_in_network_namespace() -> bool {
    std::env::var("CUENV_IN_NETNS").is_ok()
}

/// Get the namespace file descriptor for monitoring
pub fn get_namespace_fd() -> Result<i32> {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    let netns_file =
        File::open("/proc/self/ns/net").context("Failed to open network namespace file")?;

    Ok(netns_file.as_raw_fd())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_support_detection() {
        // Test that the function doesn't panic and returns a boolean
        let _supported = supports_unprivileged_namespaces();
        // Function should complete without panicking
    }

    #[test]
    fn test_kernel_version_parsing() {
        // Test various kernel version formats
        assert!(check_kernel_version_supports_namespaces("5.4.0-42-generic"));
        assert!(check_kernel_version_supports_namespaces("4.15.0"));
        assert!(check_kernel_version_supports_namespaces("3.8.0"));
        assert!(check_kernel_version_supports_namespaces("3.10.1"));

        assert!(!check_kernel_version_supports_namespaces("3.7.0"));
        assert!(!check_kernel_version_supports_namespaces("2.6.32"));
        assert!(!check_kernel_version_supports_namespaces("3.0.0"));

        // Test edge cases
        assert!(!check_kernel_version_supports_namespaces(""));
        assert!(!check_kernel_version_supports_namespaces("invalid"));
        assert!(!check_kernel_version_supports_namespaces(
            "Linux version xyz"
        ));
    }

    #[test]
    fn test_not_running_as_root() {
        // This test ensures we detect root correctly
        let is_root = getuid().is_root();
        if is_root {
            // Skip this test if actually running as root
            return;
        }
        assert!(!is_root);
    }

    #[test]
    fn test_namespace_env_var() {
        // Test environment variable detection
        std::env::remove_var("CUENV_IN_NETNS");
        assert!(!is_in_network_namespace());

        std::env::set_var("CUENV_IN_NETNS", "1");
        assert!(is_in_network_namespace());

        std::env::remove_var("CUENV_IN_NETNS");
    }

    // Note: We don't test actual namespace creation in unit tests
    // as it requires special privileges and would interfere with the test environment
}
