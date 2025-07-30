//! Memory management for cache system
//!
//! Handles memory pressure, disk quotas, and resource limits
//! with production-grade reliability.

use crate::cache::errors::{CacheError, RecoveryHint, Result};
use crate::cache::eviction::EvictionPolicy;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::time::interval;

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    /// Plenty of memory available
    Low,
    /// Memory usage is moderate
    Medium,
    /// Memory usage is high, start evicting
    High,
    /// Critical memory shortage
    Critical,
}

/// Memory manager for cache system
pub struct MemoryManager {
    /// System info tracker
    system: Arc<Mutex<System>>,
    /// Current memory pressure
    pressure: Arc<RwLock<MemoryPressure>>,
    /// Memory thresholds
    thresholds: MemoryThresholds,
    /// Disk quota tracker
    disk_quota: DiskQuotaTracker,
    /// Background task handle
    monitor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct MemoryThresholds {
    /// Start evicting when memory usage exceeds this percentage
    pub high_watermark: f64,
    /// Critical memory threshold
    pub critical_watermark: f64,
    /// Target memory usage after eviction
    pub target_usage: f64,
    /// Minimum free memory to maintain (bytes)
    pub min_free_memory: u64,
}

impl Default for MemoryThresholds {
    fn default() -> Self {
        Self {
            high_watermark: 0.80,               // 80% memory usage
            critical_watermark: 0.95,           // 95% memory usage
            target_usage: 0.70,                 // Target 70% after eviction
            min_free_memory: 512 * 1024 * 1024, // 512MB minimum free
        }
    }
}

/// Disk quota tracking
pub struct DiskQuotaTracker {
    /// Base cache directory
    base_dir: PathBuf,
    /// Maximum disk usage in bytes
    max_disk_usage: AtomicU64,
    /// Current disk usage
    current_usage: AtomicU64,
    /// Per-directory quotas
    directory_quotas: RwLock<HashMap<PathBuf, u64>>,
}

impl MemoryManager {
    pub fn new(base_dir: PathBuf, max_disk_usage: u64, thresholds: MemoryThresholds) -> Self {
        let system = System::new_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            pressure: Arc::new(RwLock::new(MemoryPressure::Low)),
            thresholds,
            disk_quota: DiskQuotaTracker {
                base_dir,
                max_disk_usage: AtomicU64::new(max_disk_usage),
                current_usage: AtomicU64::new(0),
                directory_quotas: RwLock::new(HashMap::new()),
            },
            monitor_handle: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start background monitoring
    pub fn start_monitoring(self: Arc<Self>) {
        let manager = Arc::clone(&self);
        let handle = tokio::spawn(async move {
            manager.monitor_loop().await;
        });

        *self.monitor_handle.lock() = Some(handle);
    }

    /// Background monitoring loop
    async fn monitor_loop(self: Arc<Self>) {
        let mut interval = interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            if self.shutdown.load(Ordering::Acquire) {
                break;
            }

            // Update system info
            match self.update_memory_pressure() {
                Ok(pressure) => {
                    if pressure >= MemoryPressure::High {
                        tracing::warn!("Memory pressure is {:?}", pressure);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to update memory pressure: {}", e);
                }
            }

            // Update disk usage
            match self.update_disk_usage().await {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!("Failed to update disk usage: {}", e);
                }
            }
        }
    }

    /// Update memory pressure level
    fn update_memory_pressure(&self) -> Result<MemoryPressure> {
        let mut system = self.system.lock();
        system.refresh_memory();

        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let free_memory = system.free_memory();

        let usage_ratio = used_memory as f64 / total_memory as f64;

        let pressure = if free_memory < self.thresholds.min_free_memory {
            MemoryPressure::Critical
        } else if usage_ratio >= self.thresholds.critical_watermark {
            MemoryPressure::Critical
        } else if usage_ratio >= self.thresholds.high_watermark {
            MemoryPressure::High
        } else if usage_ratio >= 0.60 {
            MemoryPressure::Medium
        } else {
            MemoryPressure::Low
        };

        *self.pressure.write() = pressure;
        Ok(pressure)
    }

    /// Update disk usage statistics
    async fn update_disk_usage(&self) -> Result<()> {
        let usage = tokio::task::spawn_blocking({
            let base_dir = self.disk_quota.base_dir.clone();
            move || calculate_directory_size(&base_dir)
        })
        .await
        .map_err(|e| CacheError::Io {
            path: self.disk_quota.base_dir.clone(),
            operation: "calculate disk usage",
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
            recovery_hint: RecoveryHint::CheckDiskSpace,
        })?;

        match usage {
            Ok(size) => {
                self.disk_quota.current_usage.store(size, Ordering::Release);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Get current memory pressure
    pub fn memory_pressure(&self) -> MemoryPressure {
        *self.pressure.read()
    }

    /// Check if we can allocate memory
    pub fn can_allocate(&self, size: u64) -> bool {
        let pressure = *self.pressure.read();

        match pressure {
            MemoryPressure::Low => true,
            MemoryPressure::Medium => size < 10 * 1024 * 1024, // Allow up to 10MB
            MemoryPressure::High => size < 1024 * 1024,        // Allow up to 1MB
            MemoryPressure::Critical => false,                 // No new allocations
        }
    }

    /// Check disk quota
    pub fn check_disk_quota(&self, size: u64) -> Result<bool> {
        let current = self.disk_quota.current_usage.load(Ordering::Acquire);
        let max = self.disk_quota.max_disk_usage.load(Ordering::Acquire);

        if current + size > max {
            Err(CacheError::DiskQuotaExceeded {
                current,
                requested: size,
                limit: max,
                recovery_hint: RecoveryHint::RunEviction,
            })
        } else {
            Ok(true)
        }
    }

    /// Record disk space usage
    pub fn record_disk_usage(&self, path: &Path, size: i64) {
        // Update total usage
        if size > 0 {
            self.disk_quota
                .current_usage
                .fetch_add(size as u64, Ordering::AcqRel);
        } else {
            self.disk_quota
                .current_usage
                .fetch_sub((-size) as u64, Ordering::AcqRel);
        }

        // Update per-directory quota if needed
        if let Some(parent) = path.parent() {
            let mut quotas = self.disk_quota.directory_quotas.write();
            let current = quotas.entry(parent.to_path_buf()).or_insert(0);

            if size > 0 {
                *current += size as u64;
            } else {
                *current = current.saturating_sub((-size) as u64);
            }
        }
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> MemoryStats {
        let system = self.system.lock();

        MemoryStats {
            total_memory: system.total_memory(),
            used_memory: system.used_memory(),
            free_memory: system.free_memory(),
            cache_memory: system.free_swap(), // Approximation
            pressure: *self.pressure.read(),
        }
    }

    /// Get disk usage statistics
    pub fn disk_stats(&self) -> DiskStats {
        DiskStats {
            total_quota: self.disk_quota.max_disk_usage.load(Ordering::Acquire),
            used_space: self.disk_quota.current_usage.load(Ordering::Acquire),
            free_space: self
                .disk_quota
                .max_disk_usage
                .load(Ordering::Acquire)
                .saturating_sub(self.disk_quota.current_usage.load(Ordering::Acquire)),
        }
    }

    /// Shutdown memory manager
    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.monitor_handle.lock().take() {
            let _ = handle.await;
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_memory: u64,
    pub used_memory: u64,
    pub free_memory: u64,
    pub cache_memory: u64,
    pub pressure: MemoryPressure,
}

/// Disk usage statistics
#[derive(Debug, Clone)]
pub struct DiskStats {
    pub total_quota: u64,
    pub used_space: u64,
    pub free_space: u64,
}

/// Calculate directory size recursively
fn calculate_directory_size(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    let entries = std::fs::read_dir(path).map_err(|e| CacheError::Io {
        path: path.to_path_buf(),
        operation: "read directory",
        source: e,
        recovery_hint: RecoveryHint::CheckPermissions {
            path: path.to_path_buf(),
        },
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to read metadata for {:?}: {}", entry.path(), e);
                continue;
            }
        };

        if metadata.is_file() {
            total_size += metadata.len();
        } else if metadata.is_dir() {
            match calculate_directory_size(&entry.path()) {
                Ok(size) => total_size += size,
                Err(e) => {
                    tracing::warn!("Failed to calculate size of {:?}: {}", entry.path(), e);
                }
            }
        }
    }

    Ok(total_size)
}

/// Cache warmer for preloading entries
pub struct CacheWarmer {
    /// Entries to warm
    warm_list: Arc<RwLock<Vec<String>>>,
    /// Warming in progress
    warming: Arc<AtomicBool>,
    /// Last warm time
    last_warm: Arc<Mutex<Option<Instant>>>,
}

impl CacheWarmer {
    pub fn new() -> Self {
        Self {
            warm_list: Arc::new(RwLock::new(Vec::new())),
            warming: Arc::new(AtomicBool::new(false)),
            last_warm: Arc::new(Mutex::new(None)),
        }
    }

    /// Add entry to warm list
    pub fn add_to_warm_list(&self, key: String) {
        self.warm_list.write().push(key);
    }

    /// Start cache warming
    pub async fn warm_cache<F, Fut>(&self, loader: F) -> Result<()>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        if self
            .warming
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(()); // Already warming
        }

        let keys = self.warm_list.read().clone();

        for key in keys {
            match loader(key.clone()).await {
                Ok(()) => {
                    tracing::debug!("Warmed cache entry: {}", key);
                }
                Err(e) => {
                    tracing::warn!("Failed to warm cache entry {}: {}", key, e);
                }
            }
        }

        *self.last_warm.lock() = Some(Instant::now());
        self.warming.store(false, Ordering::Release);

        Ok(())
    }

    /// Check if warming is needed
    pub fn needs_warming(&self, interval: Duration) -> bool {
        if self.warming.load(Ordering::Acquire) {
            return false;
        }

        match *self.last_warm.lock() {
            Some(last) => last.elapsed() > interval,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_pressure_detection() {
        let temp_dir = TempDir::new().unwrap();
        let manager = Arc::new(MemoryManager::new(
            temp_dir.path().to_path_buf(),
            1024 * 1024 * 1024, // 1GB quota
            MemoryThresholds::default(),
        ));

        // Initial pressure should be low (in tests)
        let pressure = manager.update_memory_pressure().unwrap();
        assert!(matches!(
            pressure,
            MemoryPressure::Low | MemoryPressure::Medium
        ));
    }

    #[test]
    fn test_disk_quota_checking() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(
            temp_dir.path().to_path_buf(),
            1000, // 1KB quota for testing
            MemoryThresholds::default(),
        );

        // Should allow small allocation
        assert!(manager.check_disk_quota(500).unwrap());

        // Record usage
        manager.record_disk_usage(&temp_dir.path().join("test"), 800);

        // Should fail large allocation
        assert!(manager.check_disk_quota(500).is_err());
    }

    #[tokio::test]
    async fn test_cache_warming() {
        let warmer = CacheWarmer::new();

        warmer.add_to_warm_list("key1".to_string());
        warmer.add_to_warm_list("key2".to_string());

        let mut warmed = Vec::new();
        let warmed_ref = &mut warmed;

        warmer
            .warm_cache(|key| async move {
                warmed_ref.push(key);
                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(warmed.len(), 2);
        assert!(warmed.contains(&"key1".to_string()));
        assert!(warmed.contains(&"key2".to_string()));
    }
}
