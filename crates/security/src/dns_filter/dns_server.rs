//! DNS server implementation for hostname filtering

use anyhow::Result;

use super::DnsFilter;

/// Simplified DNS server stub for now
/// TODO: Implement full DNS server using hickory-dns when build issues are resolved
pub struct FilteringDnsServer {
    filter: DnsFilter,
}

impl FilteringDnsServer {
    /// Create a new filtering DNS server (stub implementation)
    pub fn new(filter: DnsFilter) -> Result<Self> {
        log::warn!("DNS server is a stub implementation - not yet functional");
        log::debug!(
            "DNS server configured with {} allowed domains",
            filter.domain_count()
        );
        Ok(Self { filter })
    }

    /// Start serving DNS requests (stub implementation)
    pub async fn serve(&self) -> Result<()> {
        log::warn!("DNS server serve() is not yet implemented");
        log::debug!(
            "Would filter DNS requests using {} allowed domains",
            self.filter.domain_count()
        );
        // Stub implementation - return immediately
        Ok(())
    }
}

/// Start the DNS filtering server (stub implementation)
pub fn start_dns_server(filter: DnsFilter) -> Result<()> {
    log::info!("Starting DNS filtering server (stub implementation)");
    log::warn!("Full DNS server implementation pending - filtering not active yet");
    log::debug!(
        "DNS server will filter {} domains when implemented",
        filter.domain_count()
    );

    // For now, just return success to allow the system to continue
    // TODO: Implement proper DNS server with hickory-dns

    // Configure system DNS (stub implementation)
    configure_system_dns()?;

    Ok(())
}

/// Configure the system to use our DNS server (stub implementation)
fn configure_system_dns() -> Result<()> {
    log::debug!("DNS configuration is stubbed out");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns_filter::DnsFilter;

    #[tokio::test]
    async fn test_dns_filter_integration() {
        let filter = DnsFilter::new(vec!["allowed.com".to_string()]);

        // Test the filter logic directly
        assert!(filter.should_resolve("allowed.com"));
        assert!(!filter.should_resolve("blocked.com"));
    }

    #[test]
    fn test_dns_server_creation() {
        let filter = DnsFilter::new(vec!["test.com".to_string()]);

        // Test server creation (stub implementation should succeed)
        let result = FilteringDnsServer::new(filter);
        assert!(
            result.is_ok(),
            "DNS server creation should succeed in stub implementation"
        );
    }
}
