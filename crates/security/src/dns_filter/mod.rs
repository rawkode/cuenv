//! DNS-based network filtering using network namespaces
//!
//! This module provides true hostname-based network filtering by creating
//! isolated network namespaces and running a custom DNS server that only
//! resolves hostnames in the allowlist.

#[cfg(target_os = "linux")]
pub mod dns_server;
#[cfg(target_os = "linux")]
pub mod namespace;
#[cfg(target_os = "linux")]
pub mod resolver;

#[cfg(target_os = "linux")]
use std::collections::HashSet;

#[cfg(target_os = "linux")]
use anyhow::{bail, Result};

/// Helper function for wildcard domain matching
#[cfg(target_os = "linux")]
pub(crate) fn matches_wildcard(domain: &str, pattern: &str) -> bool {
    if domain.ends_with(pattern) {
        // Ensure it's a proper subdomain match (not partial match)
        if domain.len() > pattern.len()
            && domain.as_bytes()[domain.len() - pattern.len() - 1] == b'.'
        {
            return true;
        }
    }
    false
}

/// Main DNS filter interface
#[cfg(target_os = "linux")]
pub struct DnsFilter {
    allowed_hosts: HashSet<String>,
    wildcard_patterns: Vec<String>,
}

#[cfg(target_os = "linux")]
impl DnsFilter {
    /// Create a new DNS filter with the given allowed hosts
    pub fn new(allowed_hosts: Vec<String>) -> Self {
        let mut wildcard_patterns = Vec::new();
        let mut exact_hosts = HashSet::new();

        for host in allowed_hosts {
            if let Some(pattern) = host.strip_prefix("*.") {
                wildcard_patterns.push(pattern.to_string());
            } else {
                exact_hosts.insert(host);
            }
        }

        Self {
            allowed_hosts: exact_hosts,
            wildcard_patterns,
        }
    }

    /// Get the total number of allowed domains (exact + wildcard patterns)
    pub fn domain_count(&self) -> usize {
        self.allowed_hosts.len() + self.wildcard_patterns.len()
    }

    /// Check if a domain should be resolved
    pub fn should_resolve(&self, domain: &str) -> bool {
        // Check exact matches
        if self.allowed_hosts.contains(domain) {
            return true;
        }

        // Check wildcard patterns
        for pattern in &self.wildcard_patterns {
            if matches_wildcard(domain, pattern) {
                return true;
            }
        }

        false
    }

    /// Create network namespace and apply DNS filtering
    pub fn create_and_apply(allowed_hosts: &[String]) -> Result<()> {
        if !namespace::supports_unprivileged_namespaces() {
            bail!("Unprivileged user namespaces not supported on this system");
        }

        if allowed_hosts.is_empty() {
            log::warn!("No allowed hosts specified for network filtering");
            return Ok(());
        }

        // Create network namespace
        namespace::create_network_namespace()?;

        // Start DNS filter server
        let filter = Self::new(allowed_hosts.to_vec());
        dns_server::start_dns_server(filter)?;

        Ok(())
    }
}

/// Create and apply DNS filtering (Linux implementation)
#[cfg(target_os = "linux")]
pub fn create_and_apply(allowed_hosts: &[String]) -> Result<()> {
    DnsFilter::create_and_apply(allowed_hosts)
}

/// Create and apply DNS filtering (stub for non-Linux platforms)
#[cfg(not(target_os = "linux"))]
pub fn create_and_apply(_allowed_hosts: &[String]) -> anyhow::Result<()> {
    anyhow::bail!("DNS-based network filtering is only supported on Linux");
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::DnsFilter;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_exact_domain_match() {
        let filter = DnsFilter::new(vec!["api.github.com".to_string()]);
        assert!(filter.should_resolve("api.github.com"));
        assert!(!filter.should_resolve("github.com"));
        assert!(!filter.should_resolve("api.gitlab.com"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_wildcard_patterns() {
        let filter = DnsFilter::new(vec!["*.google.com".to_string()]);
        assert!(filter.should_resolve("mail.google.com"));
        assert!(filter.should_resolve("drive.google.com"));
        assert!(!filter.should_resolve("google.com")); // Not a subdomain
        assert!(!filter.should_resolve("facebook.com"));
        assert!(!filter.should_resolve("fakegoogle.com")); // Partial match should fail
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_multiple_patterns() {
        let filter = DnsFilter::new(vec![
            "api.github.com".to_string(),
            "*.npmjs.org".to_string(),
            "localhost".to_string(),
        ]);
        assert!(filter.should_resolve("api.github.com"));
        assert!(filter.should_resolve("registry.npmjs.org"));
        assert!(filter.should_resolve("localhost"));
        assert!(!filter.should_resolve("google.com"));
        assert!(!filter.should_resolve("github.com"));
    }
}
