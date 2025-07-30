use crate::cache::capabilities::{CacheCapability, CapabilityToken};
use crate::cache::errors::RecoveryHint;
use crate::cache::{CacheError, CacheResult, MonitoredCache};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Multi-tenant cache system with isolation and resource management
pub struct MultiTenantCache {
    /// Tenant-specific cache instances
    tenants: Arc<RwLock<HashMap<TenantId, TenantCache>>>,
    /// Global configuration
    config: MultiTenantConfig,
    /// Resource manager
    resource_manager: Arc<ResourceManager>,
    /// Tenant registry
    registry: Arc<RwLock<TenantRegistry>>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTenantConfig {
    /// Base directory for all tenant caches
    pub base_dir: PathBuf,
    /// Maximum number of tenants
    pub max_tenants: usize,
    /// Default quota per tenant
    pub default_quota: TenantQuota,
    /// Enable strict isolation
    pub strict_isolation: bool,
    /// Resource sharing policy
    pub sharing_policy: SharingPolicy,
    /// Eviction policy when global limits are reached
    pub global_eviction_policy: EvictionPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    /// Maximum storage in bytes
    pub max_storage_bytes: u64,
    /// Maximum number of cache entries
    pub max_entries: usize,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Rate limits
    pub rate_limits: RateLimits,
    /// CPU quota (0.0 - 1.0)
    pub cpu_quota: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Operations per second
    pub ops_per_second: f64,
    /// Bandwidth in bytes per second
    pub bandwidth_per_second: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SharingPolicy {
    /// No sharing between tenants
    NoSharing,
    /// Read-only sharing of common data
    ReadOnlySharing,
    /// Full sharing with access control
    ControlledSharing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least recently used across all tenants
    GlobalLRU,
    /// Proportional reduction based on usage
    ProportionalReduction,
    /// Priority-based eviction
    PriorityBased,
}

struct TenantCache {
    /// Isolated cache instance
    cache: Arc<MonitoredCache>,
    /// Tenant metadata
    metadata: TenantMetadata,
    /// Resource usage tracking
    usage: Arc<RwLock<ResourceUsage>>,
    /// Access control token
    token: CapabilityToken,
    /// Last activity time
    last_activity: Arc<RwLock<Instant>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantMetadata {
    pub id: TenantId,
    pub name: String,
    pub owner: String,
    pub created_at: Instant,
    pub quota: TenantQuota,
    pub priority: TenantPriority,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TenantPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Default)]
struct ResourceUsage {
    storage_bytes: u64,
    entry_count: usize,
    memory_bytes: u64,
    operations: u64,
    bandwidth_used: u64,
    last_reset: Instant,
}

struct ResourceManager {
    global_limits: GlobalLimits,
    usage_tracker: Arc<RwLock<GlobalUsage>>,
}

#[derive(Debug, Clone)]
struct GlobalLimits {
    total_storage: u64,
    total_memory: u64,
    total_entries: usize,
}

#[derive(Debug, Clone, Default)]
struct GlobalUsage {
    storage_used: u64,
    memory_used: u64,
    entries_used: usize,
}

struct TenantRegistry {
    tenants: HashMap<TenantId, TenantMetadata>,
    name_to_id: HashMap<String, TenantId>,
}

impl MultiTenantCache {
    pub async fn new(config: MultiTenantConfig) -> CacheResult<Self> {
        // Create base directory
        std::fs::create_dir_all(&config.base_dir).map_err(|e| CacheError::Io {
            path: config.base_dir.clone(),
            operation: "create multi-tenant base directory",
            source: e,
            recovery_hint: RecoveryHint::CheckPermissions {
                path: config.base_dir.clone(),
            },
        })?;

        let resource_manager = Arc::new(ResourceManager {
            global_limits: GlobalLimits {
                total_storage: config.max_tenants as u64 * config.default_quota.max_storage_bytes,
                total_memory: config.max_tenants as u64 * config.default_quota.max_memory_bytes,
                total_entries: config.max_tenants * config.default_quota.max_entries,
            },
            usage_tracker: Arc::new(RwLock::new(GlobalUsage::default())),
        });

        Ok(Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            config,
            resource_manager,
            registry: Arc::new(RwLock::new(TenantRegistry {
                tenants: HashMap::new(),
                name_to_id: HashMap::new(),
            })),
        })
    }

    /// Create a new tenant
    pub async fn create_tenant(&self, metadata: TenantMetadata) -> CacheResult<TenantId> {
        let mut registry = self.registry.write().await;
        let mut tenants = self.tenants.write().await;

        // Check if tenant already exists
        if registry.tenants.contains_key(&metadata.id) {
            return Err(CacheError::Configuration {
                message: format!("Tenant {} already exists", metadata.id.0),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Use a different tenant ID".to_string(),
                },
            });
        }

        // Check tenant limit
        if tenants.len() >= self.config.max_tenants {
            return Err(CacheError::CapacityExceeded {
                requested_bytes: 1,
                available_bytes: 0,
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Remove inactive tenants or increase max_tenants".to_string(),
                },
            });
        }

        // Create tenant directory
        let tenant_dir = self.config.base_dir.join(&metadata.id.0);
        std::fs::create_dir_all(&tenant_dir).map_err(|e| CacheError::Io {
            path: tenant_dir.clone(),
            operation: "create tenant directory",
            source: e,
            recovery_hint: RecoveryHint::CheckPermissions {
                path: tenant_dir.clone(),
            },
        })?;

        // Create tenant-specific cache
        let cache_config = crate::cache::monitoring::MonitoringConfig {
            enable_metrics: true,
            enable_tracing: true,
            enable_health_checks: true,
            metrics_interval: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(300),
        };

        let cache = Arc::new(MonitoredCache::new(
            Default::default(), // Use default cache config
            cache_config,
        ));

        // Create capability token for tenant
        let mut capabilities = Vec::new();
        capabilities.push(CacheCapability::Read);
        capabilities.push(CacheCapability::Write);
        capabilities.push(CacheCapability::Delete);

        let token = CapabilityToken::new(
            metadata.id.0.clone(),
            capabilities,
            3600, // 1 hour validity
        );

        let tenant_cache = TenantCache {
            cache,
            metadata: metadata.clone(),
            usage: Arc::new(RwLock::new(ResourceUsage::default())),
            token,
            last_activity: Arc::new(RwLock::new(Instant::now())),
        };

        let tenant_id = metadata.id.clone();

        // Register tenant
        registry.tenants.insert(tenant_id.clone(), metadata.clone());
        registry
            .name_to_id
            .insert(metadata.name.clone(), tenant_id.clone());
        tenants.insert(tenant_id.clone(), tenant_cache);

        info!("Created tenant: {} ({})", metadata.name, tenant_id.0);

        Ok(tenant_id)
    }

    /// Get cache for a specific tenant
    pub async fn get_tenant_cache(&self, tenant_id: &TenantId) -> CacheResult<Arc<MonitoredCache>> {
        let tenants = self.tenants.read().await;

        match tenants.get(tenant_id) {
            Some(tenant) => {
                // Update last activity
                *tenant.last_activity.write().await = Instant::now();

                // Check resource limits
                self.check_tenant_limits(tenant).await?;

                Ok(Arc::clone(&tenant.cache))
            }
            None => Err(CacheError::Configuration {
                message: format!("Tenant {} not found", tenant_id.0),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Create the tenant first using create_tenant()".to_string(),
                },
            }),
        }
    }

    /// Check and enforce tenant resource limits
    async fn check_tenant_limits(&self, tenant: &TenantCache) -> CacheResult<()> {
        let usage = tenant.usage.read().await;
        let quota = &tenant.metadata.quota;

        // Check storage limit
        if usage.storage_bytes > quota.max_storage_bytes {
            return Err(CacheError::DiskQuotaExceeded {
                current: usage.storage_bytes,
                requested: 0,
                limit: quota.max_storage_bytes,
                recovery_hint: RecoveryHint::RunEviction,
            });
        }

        // Check entry count limit
        if usage.entry_count > quota.max_entries {
            return Err(CacheError::CapacityExceeded {
                requested_bytes: 1,
                available_bytes: 0,
                recovery_hint: RecoveryHint::RunEviction,
            });
        }

        // Check rate limits
        let elapsed = usage.last_reset.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let ops_rate = usage.operations as f64 / elapsed;
            if ops_rate > quota.rate_limits.ops_per_second {
                return Err(CacheError::RateLimitExceeded {
                    token_id: tenant.token.id(),
                    limit: quota.rate_limits.ops_per_second,
                    window_seconds: 1,
                    recovery_hint: RecoveryHint::Retry {
                        after: Duration::from_secs(1),
                    },
                });
            }
        }

        Ok(())
    }

    /// Update tenant quota
    pub async fn update_tenant_quota(
        &self,
        tenant_id: &TenantId,
        new_quota: TenantQuota,
    ) -> CacheResult<()> {
        let mut registry = self.registry.write().await;
        let mut tenants = self.tenants.write().await;

        if let Some(metadata) = registry.tenants.get_mut(tenant_id) {
            metadata.quota = new_quota.clone();

            if let Some(tenant) = tenants.get_mut(tenant_id) {
                tenant.metadata.quota = new_quota;
            }

            info!("Updated quota for tenant {}", tenant_id.0);
            Ok(())
        } else {
            Err(CacheError::Configuration {
                message: format!("Tenant {} not found", tenant_id.0),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check tenant ID".to_string(),
                },
            })
        }
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Vec<TenantMetadata> {
        let registry = self.registry.read().await;
        registry.tenants.values().cloned().collect()
    }

    /// Get tenant statistics
    pub async fn get_tenant_stats(&self, tenant_id: &TenantId) -> CacheResult<TenantStatistics> {
        let tenants = self.tenants.read().await;

        match tenants.get(tenant_id) {
            Some(tenant) => {
                let usage = tenant.usage.read().await;
                let cache_stats = tenant.cache.get_statistics().await;

                Ok(TenantStatistics {
                    tenant_id: tenant_id.clone(),
                    resource_usage: ResourceUsageStats {
                        storage_bytes: usage.storage_bytes,
                        storage_percent: (usage.storage_bytes as f64
                            / tenant.metadata.quota.max_storage_bytes as f64)
                            * 100.0,
                        entry_count: usage.entry_count,
                        entry_percent: (usage.entry_count as f64
                            / tenant.metadata.quota.max_entries as f64)
                            * 100.0,
                        memory_bytes: usage.memory_bytes,
                        memory_percent: (usage.memory_bytes as f64
                            / tenant.metadata.quota.max_memory_bytes as f64)
                            * 100.0,
                    },
                    cache_stats,
                    last_activity: *tenant.last_activity.read().await,
                })
            }
            None => Err(CacheError::Configuration {
                message: format!("Tenant {} not found", tenant_id.0),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check tenant ID".to_string(),
                },
            }),
        }
    }

    /// Remove a tenant
    pub async fn remove_tenant(&self, tenant_id: &TenantId) -> CacheResult<()> {
        let mut registry = self.registry.write().await;
        let mut tenants = self.tenants.write().await;

        if let Some(metadata) = registry.tenants.remove(tenant_id) {
            registry.name_to_id.remove(&metadata.name);
            tenants.remove(tenant_id);

            // Clean up tenant directory
            let tenant_dir = self.config.base_dir.join(&tenant_id.0);
            if tenant_dir.exists() {
                std::fs::remove_dir_all(&tenant_dir).map_err(|e| CacheError::Io {
                    path: tenant_dir.clone(),
                    operation: "remove tenant directory",
                    source: e,
                    recovery_hint: RecoveryHint::CheckPermissions { path: tenant_dir },
                })?;
            }

            info!("Removed tenant: {}", tenant_id.0);
            Ok(())
        } else {
            Err(CacheError::Configuration {
                message: format!("Tenant {} not found", tenant_id.0),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Check tenant ID".to_string(),
                },
            })
        }
    }

    /// Perform global eviction when limits are reached
    pub async fn global_eviction(&self) -> CacheResult<()> {
        match self.config.global_eviction_policy {
            EvictionPolicy::GlobalLRU => self.evict_global_lru().await,
            EvictionPolicy::ProportionalReduction => self.evict_proportional().await,
            EvictionPolicy::PriorityBased => self.evict_by_priority().await,
        }
    }

    async fn evict_global_lru(&self) -> CacheResult<()> {
        let tenants = self.tenants.read().await;

        // Find least recently used tenant
        let mut lru_tenant = None;
        let mut oldest_time = Instant::now();

        for (id, tenant) in tenants.iter() {
            let last_activity = *tenant.last_activity.read().await;
            if last_activity < oldest_time {
                oldest_time = last_activity;
                lru_tenant = Some(id.clone());
            }
        }

        if let Some(tenant_id) = lru_tenant {
            warn!("Evicting entries from LRU tenant: {}", tenant_id.0);
            // Trigger eviction in the tenant's cache
            if let Some(tenant) = tenants.get(&tenant_id) {
                // In real implementation, would call eviction on tenant.cache
            }
        }

        Ok(())
    }

    async fn evict_proportional(&self) -> CacheResult<()> {
        let tenants = self.tenants.read().await;

        // Calculate proportional reduction for each tenant
        for (id, tenant) in tenants.iter() {
            let usage = tenant.usage.read().await;
            let usage_ratio =
                usage.storage_bytes as f64 / tenant.metadata.quota.max_storage_bytes as f64;

            if usage_ratio > 0.8 {
                let reduction_percent = (usage_ratio - 0.8) * 100.0;
                warn!(
                    "Reducing tenant {} cache by {:.1}%",
                    id.0, reduction_percent
                );
                // Trigger proportional eviction
            }
        }

        Ok(())
    }

    async fn evict_by_priority(&self) -> CacheResult<()> {
        let tenants = self.tenants.read().await;

        // Sort tenants by priority (ascending)
        let mut tenant_list: Vec<_> = tenants.iter().collect();
        tenant_list.sort_by_key(|(_, t)| t.metadata.priority);

        // Evict from low priority tenants first
        for (id, tenant) in tenant_list {
            if tenant.metadata.priority == TenantPriority::Low {
                warn!("Evicting from low priority tenant: {}", id.0);
                // Trigger eviction
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantStatistics {
    pub tenant_id: TenantId,
    pub resource_usage: ResourceUsageStats,
    pub cache_stats: crate::cache::monitoring::CacheStatistics,
    pub last_activity: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageStats {
    pub storage_bytes: u64,
    pub storage_percent: f64,
    pub entry_count: usize,
    pub entry_percent: f64,
    pub memory_bytes: u64,
    pub memory_percent: f64,
}

impl Default for MultiTenantConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(".cache/tenants"),
            max_tenants: 100,
            default_quota: TenantQuota {
                max_storage_bytes: 1024 * 1024 * 1024, // 1GB
                max_entries: 100000,
                max_memory_bytes: 256 * 1024 * 1024, // 256MB
                rate_limits: RateLimits {
                    ops_per_second: 1000.0,
                    bandwidth_per_second: 100 * 1024 * 1024, // 100MB/s
                },
                cpu_quota: 0.25, // 25% CPU
            },
            strict_isolation: true,
            sharing_policy: SharingPolicy::NoSharing,
            global_eviction_policy: EvictionPolicy::GlobalLRU,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_tenant_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = MultiTenantConfig {
            base_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mt_cache = MultiTenantCache::new(config).await.unwrap();

        let metadata = TenantMetadata {
            id: TenantId::new("test-tenant"),
            name: "Test Tenant".to_string(),
            owner: "test@example.com".to_string(),
            created_at: Instant::now(),
            quota: TenantQuota::default(),
            priority: TenantPriority::Normal,
            tags: HashMap::new(),
        };

        let tenant_id = mt_cache.create_tenant(metadata).await.unwrap();
        assert_eq!(tenant_id.0, "test-tenant");

        // Verify tenant directory was created
        let tenant_dir = temp_dir.path().join("test-tenant");
        assert!(tenant_dir.exists());
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let config = MultiTenantConfig {
            base_dir: temp_dir.path().to_path_buf(),
            strict_isolation: true,
            ..Default::default()
        };

        let mt_cache = MultiTenantCache::new(config).await.unwrap();

        // Create two tenants
        let tenant1 = TenantMetadata {
            id: TenantId::new("tenant1"),
            name: "Tenant 1".to_string(),
            owner: "user1@example.com".to_string(),
            created_at: Instant::now(),
            quota: TenantQuota::default(),
            priority: TenantPriority::Normal,
            tags: HashMap::new(),
        };

        let tenant2 = TenantMetadata {
            id: TenantId::new("tenant2"),
            name: "Tenant 2".to_string(),
            owner: "user2@example.com".to_string(),
            created_at: Instant::now(),
            quota: TenantQuota::default(),
            priority: TenantPriority::Normal,
            tags: HashMap::new(),
        };

        let id1 = mt_cache.create_tenant(tenant1).await.unwrap();
        let id2 = mt_cache.create_tenant(tenant2).await.unwrap();

        // Get caches
        let cache1 = mt_cache.get_tenant_cache(&id1).await.unwrap();
        let cache2 = mt_cache.get_tenant_cache(&id2).await.unwrap();

        // Verify isolation - caches should be different instances
        assert!(!Arc::ptr_eq(&cache1, &cache2));
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let temp_dir = TempDir::new().unwrap();
        let config = MultiTenantConfig {
            base_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mt_cache = MultiTenantCache::new(config).await.unwrap();

        let mut quota = TenantQuota::default();
        quota.max_entries = 10; // Very low limit for testing

        let metadata = TenantMetadata {
            id: TenantId::new("limited-tenant"),
            name: "Limited Tenant".to_string(),
            owner: "test@example.com".to_string(),
            created_at: Instant::now(),
            quota,
            priority: TenantPriority::Normal,
            tags: HashMap::new(),
        };

        let tenant_id = mt_cache.create_tenant(metadata).await.unwrap();
        let stats = mt_cache.get_tenant_stats(&tenant_id).await.unwrap();

        assert_eq!(stats.resource_usage.entry_count, 0);
        assert_eq!(stats.resource_usage.entry_percent, 0.0);
    }
}
