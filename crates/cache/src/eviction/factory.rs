//! Factory for creating eviction policies

use crate::errors::{CacheError, RecoveryHint, Result};

use super::policies::{ArcPolicy, LfuPolicy, LruPolicy};
use super::traits::EvictionPolicy;

/// Eviction policy factory
pub fn create_eviction_policy(
    policy_type: &str,
    max_memory: u64,
) -> Result<Box<dyn EvictionPolicy>> {
    match policy_type.to_lowercase().as_str() {
        "lru" => Ok(Box::new(LruPolicy::new(max_memory))),
        "lfu" => Ok(Box::new(LfuPolicy::new(max_memory))),
        "arc" => Ok(Box::new(ArcPolicy::new(max_memory))),
        _ => Err(CacheError::Configuration {
            message: format!("Unknown eviction policy: {policy_type}"),
            recovery_hint: RecoveryHint::UseDefault {
                value: "lru".to_string(),
            },
        }),
    }
}
