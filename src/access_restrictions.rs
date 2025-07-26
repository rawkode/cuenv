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
            Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, ABI,
        };
        use std::os::unix::process::CommandExt;

        // Check Landlock support
        let abi = ABI::V1;
        
        // We'll create a pre-exec closure that applies Landlock restrictions to the child process
        if self.restrict_disk {
            let read_only_paths = self.read_only_paths.clone();
            let read_write_paths = self.read_write_paths.clone();
            
            unsafe {
                cmd.pre_exec(move || {
                    Self::apply_landlock_filesystem_restrictions(&read_only_paths, &read_write_paths, abi)
                });
            }
        }

        if self.restrict_network {
            // Landlock v2 supports network restrictions, but it's not as comprehensive
            // For now, we'll show a more informative message about limitations
            log::warn!("Network restrictions with Landlock require kernel 5.19+ and are limited in scope");
            
            // We could add network restrictions here if we detect Landlock v2+ support
            // For now, return an error with guidance
            return Err(Error::configuration(
                "Network restrictions with Landlock are not yet fully implemented. \
                Consider using process isolation or firewall rules for network control.".to_string()
            ));
        }

        if self.restrict_process {
            // Landlock doesn't handle process restrictions directly
            // We'd need to use other mechanisms like seccomp, namespaces, or rlimits
            return Err(Error::configuration(
                "Process restrictions are not supported by Landlock. \
                Consider using system-level controls like systemd or container runtimes \
                for process isolation.".to_string()
            ));
        }

        Ok(())
    }

    /// Apply Landlock filesystem restrictions in child process
    #[cfg(target_os = "linux")]
    fn apply_landlock_filesystem_restrictions(
        read_only_paths: &[PathBuf],
        read_write_paths: &[PathBuf],
        _abi: landlock::ABI,
    ) -> std::io::Result<()> {
        use landlock::{
            Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
        };

        // If no paths are specified, don't apply any restrictions
        // This avoids the complexity of trying to determine what should be allowed by default
        if read_only_paths.is_empty() && read_write_paths.is_empty() {
            return Ok(());
        }

        // Create a new Landlock ruleset for filesystem restrictions
        let fs_access = AccessFs::Execute | AccessFs::WriteFile | AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::RemoveDir | AccessFs::RemoveFile | AccessFs::MakeChar | AccessFs::MakeDir | AccessFs::MakeReg | AccessFs::MakeSock | AccessFs::MakeFifo | AccessFs::MakeBlock | AccessFs::MakeSym;
        
        let mut ruleset = Ruleset::default()
            .handle_access(fs_access)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Landlock ruleset: {e}")))?
            .create()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Landlock ruleset: {e}")))?;

        // Add user-specified read-only paths
        for path in read_only_paths {
            let path_fd = PathFd::new(path)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to open path {}: {e}", path.display())))?;
            ruleset = ruleset
                .add_rule(PathBeneath::new(path_fd, AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to add read-only rule for {}: {e}", path.display())))?;
        }

        // Add user-specified read-write paths
        for path in read_write_paths {
            let path_fd = PathFd::new(path)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to open path {}: {e}", path.display())))?;
            ruleset = ruleset
                .add_rule(PathBeneath::new(
                    path_fd,
                    AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::WriteFile | AccessFs::RemoveFile | AccessFs::RemoveDir | AccessFs::MakeChar | AccessFs::MakeDir | AccessFs::MakeReg | AccessFs::MakeSock | AccessFs::MakeFifo | AccessFs::MakeBlock | AccessFs::MakeSym | AccessFs::Execute,
                ))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to add read-write rule for {}: {e}", path.display())))?;
        }

        // Apply the Landlock policy to this process (which will be the child)
        ruleset.restrict_self()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to apply Landlock restrictions: {e}")))?;
        
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