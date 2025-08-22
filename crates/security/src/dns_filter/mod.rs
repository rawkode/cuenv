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
use std::net::UdpSocket;
#[cfg(target_os = "linux")]
use std::thread;

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

/// Start a simple DNS filtering proxy in the current network namespace
#[cfg(target_os = "linux")]
pub fn start_dns_filter(allowed_hosts: &[String]) -> Result<()> {
    log::info!("Starting DNS filter with allowed hosts: {:?}", allowed_hosts);
    let allowed_set: HashSet<String> = allowed_hosts.iter().cloned().collect();
    
    // Start DNS proxy in background thread
    thread::spawn(move || {
        log::info!("DNS proxy thread starting...");
        if let Err(e) = run_dns_proxy(allowed_set) {
            log::error!("DNS proxy failed: {}", e);
        }
    });
    
    // Configure resolv.conf to use our proxy
    log::info!("Configuring /etc/resolv.conf to use local DNS proxy");
    
    // First try to create a temporary resolv.conf file
    let temp_resolv = "/tmp/cuenv_resolv.conf";
    std::fs::write(temp_resolv, "nameserver 127.0.0.1\n")
        .map_err(|e| anyhow::anyhow!("Failed to create temp resolv.conf: {}", e))?;
    
    // Try to bind mount it over /etc/resolv.conf
    if let Err(e) = bind_mount_resolv_conf(temp_resolv) {
        log::warn!("Could not bind mount resolv.conf: {}, trying direct write", e);
        // Fall back to direct write (might not work with systemd-resolved)
        if let Err(e2) = std::fs::write("/etc/resolv.conf", "nameserver 127.0.0.1\n") {
            log::error!("Failed to configure DNS: bind mount failed ({}), direct write failed ({})", e, e2);
            return Err(anyhow::anyhow!("Failed to configure DNS resolver"));
        }
    }
    
    // Give the proxy a moment to start
    std::thread::sleep(std::time::Duration::from_millis(100));
    log::info!("DNS filter setup complete");
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_dns_proxy(allowed_hosts: HashSet<String>) -> Result<()> {
    // Try to bind to port 53, fall back to higher port if needed
    let socket = match UdpSocket::bind("127.0.0.1:53") {
        Ok(s) => {
            log::info!("DNS proxy bound to 127.0.0.1:53");
            s
        }
        Err(e) => {
            log::warn!("Could not bind to port 53: {}, trying port 5353", e);
            UdpSocket::bind("127.0.0.1:5353")
                .map_err(|e| anyhow::anyhow!("Failed to bind DNS proxy to any port: {}", e))?
        }
    };
    
    let upstream = "8.8.8.8:53";
    log::info!("DNS proxy started, forwarding allowed queries to {}", upstream);
    
    let mut buf = [0u8; 512];
    
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                if let Some(domain) = parse_dns_query(&buf[..len]) {
                    log::info!("DNS query for domain: {} (allowed hosts: {:?})", domain, allowed_hosts);
                    if is_domain_allowed(&domain, &allowed_hosts) {
                        log::info!("Allowing DNS query for: {}", domain);
                        // Forward to upstream DNS
                        if let Ok(upstream_socket) = UdpSocket::bind("0.0.0.0:0") {
                            if upstream_socket.send_to(&buf[..len], upstream).is_ok() {
                                let mut response = [0u8; 512];
                                if let Ok((rlen, _)) = upstream_socket.recv_from(&mut response) {
                                    log::debug!("Forwarded DNS response for: {}", domain);
                                    let _ = socket.send_to(&response[..rlen], src);
                                }
                            }
                        }
                    } else {
                        // Send NXDOMAIN for blocked domains
                        log::warn!("Blocking DNS query for: {} (not in allowed list)", domain);
                        let nxdomain = build_nxdomain_response(&buf[..len]);
                        let _ = socket.send_to(&nxdomain, src);
                    }
                } else {
                    log::warn!("Could not parse DNS query from {} byte packet", len);
                }
            }
            Err(e) => {
                log::error!("DNS proxy recv error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn parse_dns_query(packet: &[u8]) -> Option<String> {
    // Very basic DNS query parsing
    if packet.len() < 12 {
        return None;
    }
    
    // Skip DNS header (12 bytes)
    let mut pos = 12;
    let mut domain = String::new();
    
    // Parse QNAME (domain name in DNS wire format)
    while pos < packet.len() {
        let len = packet[pos] as usize;
        if len == 0 {
            break; // End of domain
        }
        
        pos += 1;
        if pos + len > packet.len() {
            return None;
        }
        
        if !domain.is_empty() {
            domain.push('.');
        }
        
        // Extract label
        for i in 0..len {
            if pos + i < packet.len() {
                domain.push(packet[pos + i] as char);
            }
        }
        
        pos += len;
    }
    
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

#[cfg(target_os = "linux")]
fn is_domain_allowed(domain: &str, allowed_hosts: &HashSet<String>) -> bool {
    // Check exact match
    if allowed_hosts.contains(domain) {
        return true;
    }
    
    // Check wildcard patterns
    for pattern in allowed_hosts {
        if pattern.starts_with("*.") {
            let suffix = &pattern[2..];
            if domain.ends_with(suffix) {
                return true;
            }
        }
    }
    
    false
}

#[cfg(target_os = "linux")]
fn build_nxdomain_response(query: &[u8]) -> Vec<u8> {
    if query.len() < 12 {
        return Vec::new();
    }
    
    let mut response = query.to_vec();
    
    // Set QR bit (response) and RCODE to NXDOMAIN (3)
    if response.len() >= 3 {
        response[2] |= 0x80; // QR bit
        response[3] = (response[3] & 0xF0) | 0x03; // RCODE = 3 (NXDOMAIN)
    }
    
    response
}

/// Create and apply DNS filtering (stub for non-Linux platforms)
#[cfg(not(target_os = "linux"))]
pub fn create_and_apply(_allowed_hosts: &[String]) -> anyhow::Result<()> {
    anyhow::bail!("DNS-based network filtering is only supported on Linux");
}

/// Bind mount a custom resolv.conf over /etc/resolv.conf
#[cfg(target_os = "linux")]
fn bind_mount_resolv_conf(source: &str) -> Result<()> {
    use std::process::Command;
    
    // Use mount command to bind mount our resolv.conf
    let output = Command::new("mount")
        .args(["--bind", source, "/etc/resolv.conf"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run mount command: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Mount failed: {}", stderr));
    }
    
    log::info!("Successfully bind mounted custom resolv.conf");
    Ok(())
}

/// Start DNS filter (stub for non-Linux platforms)
#[cfg(not(target_os = "linux"))]
pub fn start_dns_filter(_allowed_hosts: &[String]) -> anyhow::Result<()> {
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
