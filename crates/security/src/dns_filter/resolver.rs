//! Domain matching and resolution logic

use std::collections::HashSet;

/// Domain resolution logic for the DNS filter
pub struct DomainResolver {
    exact_hosts: HashSet<String>,
    wildcard_patterns: Vec<String>,
}

impl DomainResolver {
    /// Create a new domain resolver
    pub fn new(allowed_hosts: Vec<String>) -> Self {
        let mut exact_hosts = HashSet::new();
        let mut wildcard_patterns = Vec::new();

        for host in allowed_hosts {
            let normalized_host = host.trim_end_matches('.').to_lowercase();
            if let Some(pattern) = normalized_host.strip_prefix("*.") {
                // Extract the domain part after the wildcard
                wildcard_patterns.push(pattern.to_string());
            } else {
                exact_hosts.insert(normalized_host);
            }
        }

        Self {
            exact_hosts,
            wildcard_patterns,
        }
    }

    /// Check if a domain should be resolved
    pub fn should_resolve(&self, domain: &str) -> bool {
        // Normalize domain (remove trailing dot, convert to lowercase)
        let normalized = domain.trim_end_matches('.').to_lowercase();

        // Check exact matches
        if self.exact_hosts.contains(&normalized) {
            return true;
        }

        // Check wildcard patterns
        for pattern in &self.wildcard_patterns {
            if self.matches_wildcard(&normalized, pattern) {
                return true;
            }
        }

        false
    }

    /// Check if a domain matches a wildcard pattern
    fn matches_wildcard(&self, domain: &str, pattern: &str) -> bool {
        // Pattern should match subdomains only, not the domain itself
        if domain == pattern {
            return false;
        }

        // Check if domain ends with the pattern
        if domain.ends_with(pattern) {
            // Ensure there's a dot before the pattern (proper subdomain)
            let prefix_len = domain.len() - pattern.len();
            if prefix_len > 0 {
                let prefix = &domain[..prefix_len];
                return prefix.ends_with('.');
            }
        }

        false
    }

    /// Add a new allowed host at runtime
    pub fn add_allowed_host(&mut self, host: String) {
        if let Some(pattern) = host.strip_prefix("*.") {
            self.wildcard_patterns.push(pattern.to_string());
        } else {
            self.exact_hosts.insert(host);
        }
    }

    /// Remove an allowed host at runtime
    pub fn remove_allowed_host(&mut self, host: &str) {
        if let Some(pattern) = host.strip_prefix("*.") {
            self.wildcard_patterns.retain(|p| p != pattern);
        } else {
            self.exact_hosts.remove(host);
        }
    }

    /// Get all exact hosts
    pub fn get_exact_hosts(&self) -> &HashSet<String> {
        &self.exact_hosts
    }

    /// Get all wildcard patterns
    pub fn get_wildcard_patterns(&self) -> &[String] {
        &self.wildcard_patterns
    }

    /// Check if any hosts are configured
    pub fn has_allowed_hosts(&self) -> bool {
        !self.exact_hosts.is_empty() || !self.wildcard_patterns.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_domain_matching() {
        let resolver =
            DomainResolver::new(vec!["api.github.com".to_string(), "localhost".to_string()]);

        assert!(resolver.should_resolve("api.github.com"));
        assert!(resolver.should_resolve("localhost"));
        assert!(!resolver.should_resolve("github.com"));
        assert!(!resolver.should_resolve("api.gitlab.com"));
    }

    #[test]
    fn test_wildcard_matching() {
        let resolver =
            DomainResolver::new(vec!["*.google.com".to_string(), "*.npmjs.org".to_string()]);

        // Should match subdomains
        assert!(resolver.should_resolve("mail.google.com"));
        assert!(resolver.should_resolve("drive.google.com"));
        assert!(resolver.should_resolve("registry.npmjs.org"));

        // Should not match the domain itself
        assert!(!resolver.should_resolve("google.com"));
        assert!(!resolver.should_resolve("npmjs.org"));

        // Should not match partial matches
        assert!(!resolver.should_resolve("fakegoogle.com"));
        assert!(!resolver.should_resolve("google.com.evil.com"));
    }

    #[test]
    fn test_case_insensitive_matching() {
        let resolver = DomainResolver::new(vec![
            "API.GitHub.COM".to_string(),
            "*.GOOGLE.COM".to_string(),
        ]);

        assert!(resolver.should_resolve("api.github.com"));
        assert!(resolver.should_resolve("API.GITHUB.COM"));
        assert!(resolver.should_resolve("mail.google.com"));
        assert!(resolver.should_resolve("MAIL.GOOGLE.COM"));
    }

    #[test]
    fn test_trailing_dot_normalization() {
        let resolver = DomainResolver::new(vec!["example.com".to_string()]);

        assert!(resolver.should_resolve("example.com"));
        assert!(resolver.should_resolve("example.com."));
    }

    #[test]
    fn test_dynamic_host_management() {
        let mut resolver = DomainResolver::new(vec![]);

        assert!(!resolver.has_allowed_hosts());

        resolver.add_allowed_host("test.com".to_string());
        assert!(resolver.has_allowed_hosts());
        assert!(resolver.should_resolve("test.com"));

        resolver.add_allowed_host("*.example.com".to_string());
        assert!(resolver.should_resolve("sub.example.com"));

        resolver.remove_allowed_host("test.com");
        assert!(!resolver.should_resolve("test.com"));
        assert!(resolver.should_resolve("sub.example.com"));

        resolver.remove_allowed_host("*.example.com");
        assert!(!resolver.should_resolve("sub.example.com"));
    }

    #[test]
    fn test_complex_wildcard_scenarios() {
        let resolver = DomainResolver::new(vec![
            "*.github.com".to_string(),
            "api.special.com".to_string(),
        ]);

        // Wildcard should work
        assert!(resolver.should_resolve("api.github.com"));
        assert!(!resolver.should_resolve("raw.githubusercontent.com")); // Different domain

        // Exact match should work
        assert!(resolver.should_resolve("api.special.com"));

        // Should not match the base domain of wildcard
        assert!(!resolver.should_resolve("github.com"));

        // Should not match subdomain of exact match
        assert!(!resolver.should_resolve("sub.api.special.com"));
    }

    #[test]
    fn test_empty_resolver() {
        let resolver = DomainResolver::new(vec![]);

        assert!(!resolver.has_allowed_hosts());
        assert!(!resolver.should_resolve("any.domain.com"));
        assert!(!resolver.should_resolve("localhost"));
    }
}
