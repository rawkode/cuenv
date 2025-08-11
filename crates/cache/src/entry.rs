//! Cache entry management and in-memory structures

use crate::traits::CacheMetadata;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// In-memory cache entry
#[derive(Debug, Clone)]
pub struct InMemoryEntry {
    /// Serialized data
    pub data: Arc<Vec<u8>>,
    /// Entry metadata
    pub metadata: CacheMetadata,
    /// Creation timestamp
    pub created_at: SystemTime,
    /// TTL for this entry
    pub ttl: Option<Duration>,
}

impl InMemoryEntry {
    pub fn new(data: Vec<u8>, ttl: Option<Duration>) -> Self {
        let now = SystemTime::now();
        let size_bytes = data.len() as u64;
        Self {
            data: Arc::new(data),
            metadata: CacheMetadata {
                size_bytes,
                created_at: now,
                last_accessed: now,
                expires_at: ttl.map(|t| now + t),
                access_count: 0,
                content_hash: String::new(),
                cache_version: 1,
            },
            created_at: now,
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.ttl {
            Some(ttl) => {
                match self.created_at.elapsed() {
                    Ok(elapsed) => elapsed > ttl,
                    Err(_) => true, // Clock moved backwards, consider expired
                }
            }
            None => false,
        }
    }

    pub fn deserialize<T: DeserializeOwned>(
        &self,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        bincode::deserialize(&self.data).map_err(Into::into)
    }
}

/// Cache entry statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub memory_usage: u64,
    pub disk_usage: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
