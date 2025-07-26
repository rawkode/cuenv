use crate::access_restrictions::AccessRestrictions;
use std::path::PathBuf;

/// Builder for creating AccessRestrictions with a fluent API
#[derive(Debug, Default)]
pub struct AccessRestrictionsBuilder {
    restrict_disk: bool,
    restrict_network: bool,
    read_only_paths: Vec<PathBuf>,
    read_write_paths: Vec<PathBuf>,
    deny_paths: Vec<PathBuf>,
    allowed_hosts: Vec<String>,
    audit_mode: bool,
}

impl AccessRestrictionsBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable disk restrictions
    pub fn restrict_disk(mut self, restrict: bool) -> Self {
        self.restrict_disk = restrict;
        self
    }

    /// Enable network restrictions
    pub fn restrict_network(mut self, restrict: bool) -> Self {
        self.restrict_network = restrict;
        self
    }

    /// Add a read-only path
    pub fn add_read_only_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.read_only_paths.push(path.into());
        self
    }

    /// Add multiple read-only paths
    pub fn read_only_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.read_only_paths
            .extend(paths.into_iter().map(|p| p.into()));
        self
    }

    /// Add a read-write path
    pub fn add_read_write_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.read_write_paths.push(path.into());
        self
    }

    /// Add multiple read-write paths
    pub fn read_write_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.read_write_paths
            .extend(paths.into_iter().map(|p| p.into()));
        self
    }

    /// Add a deny path
    pub fn add_deny_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.deny_paths.push(path.into());
        self
    }

    /// Add multiple deny paths
    pub fn deny_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.deny_paths.extend(paths.into_iter().map(|p| p.into()));
        self
    }

    /// Add an allowed host
    pub fn add_allowed_host<S: Into<String>>(mut self, host: S) -> Self {
        self.allowed_hosts.push(host.into());
        self
    }

    /// Add multiple allowed hosts
    pub fn allowed_hosts<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_hosts
            .extend(hosts.into_iter().map(|h| h.into()));
        self
    }

    /// Enable audit mode
    pub fn audit_mode(mut self, enable: bool) -> Self {
        self.audit_mode = enable;
        self
    }

    /// Build the AccessRestrictions
    pub fn build(self) -> AccessRestrictions {
        let mut restrictions = AccessRestrictions::with_allowlists(
            self.restrict_disk,
            self.restrict_network,
            self.read_only_paths,
            self.read_write_paths,
            self.deny_paths,
            self.allowed_hosts,
        );

        if self.audit_mode {
            restrictions.enable_audit_mode();
        }

        restrictions
    }
}

impl AccessRestrictions {
    /// Create a new builder for AccessRestrictions
    pub fn builder() -> AccessRestrictionsBuilder {
        AccessRestrictionsBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let restrictions = AccessRestrictions::builder()
            .restrict_disk(true)
            .restrict_network(false)
            .build();

        assert!(restrictions.restrict_disk);
        assert!(!restrictions.restrict_network);
    }

    #[test]
    fn test_builder_with_paths() {
        let restrictions = AccessRestrictions::builder()
            .restrict_disk(true)
            .add_read_only_path("/etc")
            .add_read_only_path("/usr")
            .add_read_write_path("/tmp")
            .add_deny_path("/etc/passwd")
            .build();

        assert_eq!(restrictions.read_only_paths.len(), 2);
        assert_eq!(restrictions.read_write_paths.len(), 1);
        assert_eq!(restrictions.deny_paths.len(), 1);
    }

    #[test]
    fn test_builder_with_multiple_paths() {
        let restrictions = AccessRestrictions::builder()
            .restrict_disk(true)
            .read_only_paths(vec!["/etc", "/usr", "/lib"])
            .read_write_paths(vec!["/tmp", "/var/tmp"])
            .build();

        assert_eq!(restrictions.read_only_paths.len(), 3);
        assert_eq!(restrictions.read_write_paths.len(), 2);
    }

    #[test]
    fn test_builder_with_hosts() {
        let restrictions = AccessRestrictions::builder()
            .restrict_network(true)
            .add_allowed_host("example.com")
            .add_allowed_host("*.internal.net")
            .allowed_hosts(vec!["10.0.0.0/8", "192.168.0.0/16"])
            .build();

        assert_eq!(restrictions.allowed_hosts.len(), 4);
    }

    #[test]
    fn test_builder_audit_mode() {
        let restrictions = AccessRestrictions::builder()
            .restrict_disk(true)
            .audit_mode(true)
            .build();

        assert!(restrictions.audit_mode);
    }

    #[test]
    fn test_builder_fluent_api() {
        let restrictions = AccessRestrictions::builder()
            .restrict_disk(true)
            .restrict_network(true)
            .add_read_only_path("/etc")
            .add_read_write_path("/tmp")
            .add_allowed_host("example.com")
            .audit_mode(false)
            .build();

        assert!(restrictions.restrict_disk);
        assert!(restrictions.restrict_network);
        assert_eq!(restrictions.read_only_paths.len(), 1);
        assert_eq!(restrictions.read_write_paths.len(), 1);
        assert_eq!(restrictions.allowed_hosts.len(), 1);
        assert!(!restrictions.audit_mode);
    }
}
