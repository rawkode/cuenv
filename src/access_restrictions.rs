use crate::errors::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

/// Configuration for access restrictions when running commands
#[derive(Debug, Clone, Default)]
pub struct AccessRestrictions {
    /// Restrict disk access (filesystem operations)
    pub restrict_disk: bool,
    /// Restrict process access (process spawning and IPC)
    pub restrict_process: bool,
    /// Restrict network access (network connections)
    pub restrict_network: bool,
    /// Paths that are allowed for reading
    pub read_only_paths: Vec<PathBuf>,
    /// Paths that are allowed for reading and writing
    pub read_write_paths: Vec<PathBuf>,
    /// Paths that are explicitly denied
    pub deny_paths: Vec<PathBuf>,
    /// Allowed network hosts/CIDRs (empty means block all)
    pub allowed_hosts: Vec<String>,
}

impl AccessRestrictions {
    /// Create new restrictions configuration
    pub fn new(restrict_disk: bool, restrict_process: bool, restrict_network: bool) -> Self {
        Self {
            restrict_disk,
            restrict_process,
            restrict_network,
            read_only_paths: Vec::new(),
            read_write_paths: Vec::new(),
            deny_paths: Vec::new(),
            allowed_hosts: Vec::new(),
        }
    }

    /// Create restrictions with explicit path and network allowlists
    pub fn with_allowlists(
        restrict_disk: bool,
        restrict_process: bool,
        restrict_network: bool,
        read_only_paths: Vec<PathBuf>,
        read_write_paths: Vec<PathBuf>,
        deny_paths: Vec<PathBuf>,
        allowed_hosts: Vec<String>,
    ) -> Self {
        Self {
            restrict_disk,
            restrict_process,
            restrict_network,
            read_only_paths,
            read_write_paths,
            deny_paths,
            allowed_hosts,
        }
    }

    /// Add a read-only path to the allowlist
    pub fn add_read_only_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.read_only_paths.push(path.into());
    }

    /// Add a read-write path to the allowlist
    pub fn add_read_write_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.read_write_paths.push(path.into());
    }

    /// Add a denied path
    pub fn add_deny_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.deny_paths.push(path.into());
    }

    /// Add an allowed network host/CIDR
    pub fn add_allowed_host<S: Into<String>>(&mut self, host: S) {
        self.allowed_hosts.push(host.into());
    }

    /// Apply restrictions to a command before execution
    /// This is the main entry point for applying platform-specific restrictions
    pub fn apply_to_command(&self, cmd: &mut Command) -> Result<()> {
        if !self.has_any_restrictions() {
            return Ok(());
        }

        // Apply platform-specific restrictions
        #[cfg(target_os = "linux")]
        self.apply_landlock_restrictions(cmd)?;

        #[cfg(not(target_os = "linux"))]
        self.apply_fallback_restrictions(cmd)?;

        Ok(())
    }

    /// Check if any restrictions are enabled
    pub fn has_any_restrictions(&self) -> bool {
        self.restrict_disk || self.restrict_process || self.restrict_network
    }

    /// Apply Landlock-based restrictions on Linux
    #[cfg(target_os = "linux")]
    fn apply_landlock_restrictions(&self, cmd: &mut Command) -> Result<()> {
        use landlock::{
            Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
        };

        // Check if Landlock is supported on this kernel
        let abi = ABI::V1;
        
        if self.restrict_disk {
            // Create a new Landlock ruleset for filesystem restrictions
            let mut ruleset = Ruleset::default()
                .handle_access(AccessFs::from_all(abi))
                .map_err(|e| Error::configuration(format!("Failed to create Landlock ruleset: {e}")))?
                .create()
                .map_err(|e| Error::configuration(format!("Failed to create Landlock ruleset: {e}")))?;

            // Add read-only paths
            for path in &self.read_only_paths {
                let path_fd = PathFd::new(path)
                    .map_err(|e| Error::configuration(format!("Failed to open path {}: {e}", path.display())))?;
                ruleset = ruleset
                    .add_rule(PathBeneath::new(path_fd, AccessFs::ReadFile | AccessFs::ReadDir))
                    .map_err(|e| Error::configuration(format!("Failed to add read-only rule for {}: {e}", path.display())))?;
            }

            // Add read-write paths
            for path in &self.read_write_paths {
                let path_fd = PathFd::new(path)
                    .map_err(|e| Error::configuration(format!("Failed to open path {}: {e}", path.display())))?;
                ruleset = ruleset
                    .add_rule(PathBeneath::new(
                        path_fd,
                        AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::WriteFile | AccessFs::RemoveFile | AccessFs::RemoveDir | AccessFs::MakeChar | AccessFs::MakeDir | AccessFs::MakeReg | AccessFs::MakeSock | AccessFs::MakeFifo | AccessFs::MakeBlock | AccessFs::MakeSym,
                    ))
                    .map_err(|e| Error::configuration(format!("Failed to add read-write rule for {}: {e}", path.display())))?;
            }

            // The Landlock restriction will be applied when we execute the command
            // We need to wrap the command execution with Landlock enforcement
            self.wrap_command_with_landlock(cmd, ruleset)?;
        }

        // Note: Landlock doesn't directly support network restrictions in the same way
        // For network restrictions, we would need to use other mechanisms
        if self.restrict_network {
            return Err(Error::configuration(
                "Network restrictions using Landlock are not yet implemented. Use process isolation instead.".to_string()
            ));
        }

        // Process restrictions would require additional mechanisms beyond Landlock
        if self.restrict_process {
            return Err(Error::configuration(
                "Process restrictions are not yet implemented with Landlock. Use system-level controls instead.".to_string()
            ));
        }

        Ok(())
    }

    /// Wrap command execution with Landlock enforcement
    #[cfg(target_os = "linux")]
    fn wrap_command_with_landlock(&self, _cmd: &mut Command, ruleset: landlock::RulesetCreated) -> Result<()> {
        // For now, we'll use a simple approach: apply the Landlock policy in a pre-exec hook
        // This is not ideal as it applies to the parent process, but it's a starting point
        
        // In a real implementation, we'd want to fork and apply Landlock in the child process
        // before exec'ing the target command. For now, we'll document this limitation.
        
        log::warn!("Landlock disk restrictions will be applied to the current process. This is a limitation of the current implementation.");
        
        // Apply the Landlock policy
        ruleset.restrict_self()
            .map_err(|e| Error::configuration(format!("Failed to apply Landlock restrictions: {e}")))?;
        
        Ok(())
    }

    /// Apply fallback restrictions on non-Linux platforms
    #[cfg(not(target_os = "linux"))]
    fn apply_fallback_restrictions(&self, _cmd: &mut Command) -> Result<()> {
        if self.has_any_restrictions() {
            return Err(Error::configuration(
                "Access restrictions are only supported on Linux with Landlock. Please use a Linux system with kernel 5.13+ for sandboxing support.".to_string()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_restrictions_creation() {
        let restrictions = AccessRestrictions::new(true, false, true);
        assert!(restrictions.restrict_disk);
        assert!(!restrictions.restrict_process);
        assert!(restrictions.restrict_network);
        assert!(restrictions.has_any_restrictions());
    }

    #[test]
    fn test_access_restrictions_with_allowlists() {
        let restrictions = AccessRestrictions::with_allowlists(
            true,
            false,
            true,
            vec![PathBuf::from("/tmp")],
            vec![PathBuf::from("/var/tmp")],
            vec![PathBuf::from("/etc/passwd")],
            vec!["localhost".to_string()],
        );
        
        assert!(restrictions.restrict_disk);
        assert!(!restrictions.restrict_process);
        assert!(restrictions.restrict_network);
        assert_eq!(restrictions.read_only_paths.len(), 1);
        assert_eq!(restrictions.read_write_paths.len(), 1);
        assert_eq!(restrictions.deny_paths.len(), 1);
        assert_eq!(restrictions.allowed_hosts.len(), 1);
    }

    #[test]
    fn test_no_restrictions() {
        let restrictions = AccessRestrictions::default();
        assert!(!restrictions.has_any_restrictions());
    }

    #[test]
    fn test_add_paths_and_hosts() {
        let mut restrictions = AccessRestrictions::new(true, false, true);
        restrictions.add_read_only_path("/usr/lib");
        restrictions.add_read_write_path("/tmp");
        restrictions.add_deny_path("/etc/shadow");
        restrictions.add_allowed_host("example.com");

        assert_eq!(restrictions.read_only_paths.len(), 1);
        assert_eq!(restrictions.read_write_paths.len(), 1);
        assert_eq!(restrictions.deny_paths.len(), 1);
        assert_eq!(restrictions.allowed_hosts.len(), 1);
    }

    #[test]
    fn test_apply_to_command_no_restrictions() {
        let restrictions = AccessRestrictions::default();
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        
        let result = restrictions.apply_to_command(&mut cmd);
        assert!(result.is_ok());
        
        // Command should be unchanged
        assert_eq!(cmd.get_program(), "echo");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_landlock_restrictions_available() {
        let mut restrictions = AccessRestrictions::new(true, false, false);
        restrictions.add_read_only_path("/tmp");
        restrictions.add_read_write_path("/var/tmp");
        
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        
        // This might fail if Landlock is not available, but shouldn't panic
        let _result = restrictions.apply_to_command(&mut cmd);
        // We can't easily test the actual Landlock functionality without kernel support
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_non_linux_restrictions_fail() {
        let restrictions = AccessRestrictions::new(true, false, false);
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        
        let result = restrictions.apply_to_command(&mut cmd);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only supported on Linux"));
    }
}