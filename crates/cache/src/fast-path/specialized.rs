//! Specialized implementations for common value types

use super::core::FastPathCache;
use crate::traits::CacheMetadata;
use serde::Deserialize;
use std::time::{Duration, SystemTime};

/// Fast path for string values
#[inline(always)]
pub fn get_string(cache: &FastPathCache, key: &str) -> Option<String> {
    cache
        .get_small(key)
        .and_then(|(data, _)| String::from_utf8(data).ok())
}

/// Fast path for boolean flags
#[inline(always)]
pub fn get_bool(cache: &FastPathCache, key: &str) -> Option<bool> {
    cache.get_small(key).and_then(|(data, _)| {
        if data.len() == 1 {
            Some(data[0] != 0)
        } else {
            None
        }
    })
}

/// Fast path for u64 values
#[inline(always)]
pub fn get_u64(cache: &FastPathCache, key: &str) -> Option<u64> {
    cache.get_small(key).and_then(|(data, _)| {
        if data.len() == 8 {
            Some(u64::from_le_bytes(data.try_into().unwrap()))
        } else {
            None
        }
    })
}

/// Fast path for JSON values under 1KB
#[inline(always)]
pub fn get_json<T: for<'de> Deserialize<'de>>(cache: &FastPathCache, key: &str) -> Option<T> {
    cache
        .get_small(key)
        .and_then(|(data, _)| serde_json::from_slice(&data).ok())
}

/// Fast path to put string
#[inline(always)]
pub fn put_string(cache: &FastPathCache, key: String, value: &str, ttl: Option<Duration>) -> bool {
    let metadata = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: ttl.map(|d| SystemTime::now() + d),
        size_bytes: value.len() as u64,
        access_count: 0,
        content_hash: String::new(), // Skip for fast path
        cache_version: 3,
    };

    cache.put_small(key, value.as_bytes().to_vec(), metadata)
}

/// Fast path to put boolean
#[inline(always)]
pub fn put_bool(cache: &FastPathCache, key: String, value: bool, ttl: Option<Duration>) -> bool {
    let metadata = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: ttl.map(|d| SystemTime::now() + d),
        size_bytes: 1,
        access_count: 0,
        content_hash: String::new(),
        cache_version: 3,
    };

    cache.put_small(key, vec![if value { 1 } else { 0 }], metadata)
}
