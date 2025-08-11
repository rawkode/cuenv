//! Tests for fast path optimizations

use super::core::FastPathCache;
use super::inline::InlineCache;
use super::specialized;
use crate::traits::CacheMetadata;
use std::time::{Duration, SystemTime};

#[test]
fn test_fast_path_small_values() {
    let cache = FastPathCache::new(1024, 100);

    // Test string fast path
    assert!(specialized::put_string(
        &cache,
        "key1".to_string(),
        "small value",
        None
    ));

    assert_eq!(
        specialized::get_string(&cache, "key1"),
        Some("small value".to_string())
    );

    // Test bool fast path
    assert!(specialized::put_bool(
        &cache,
        "flag".to_string(),
        true,
        None
    ));

    assert_eq!(specialized::get_bool(&cache, "flag"), Some(true));
}

#[test]
fn test_inline_cache() {
    let mut cache = InlineCache::<4>::new();

    cache.put("key1".to_string(), b"value1".to_vec());
    cache.put("key2".to_string(), b"value2".to_vec());

    assert_eq!(cache.get("key1"), Some(b"value1".as_ref()));
    assert_eq!(cache.get("key2"), Some(b"value2".as_ref()));
    assert_eq!(cache.get("key3"), None);

    // Test wraparound
    cache.put("key3".to_string(), b"value3".to_vec());
    cache.put("key4".to_string(), b"value4".to_vec());
    cache.put("key5".to_string(), b"value5".to_vec()); // Overwrites key1

    assert_eq!(cache.get("key1"), None); // Evicted
    assert_eq!(cache.get("key5"), Some(b"value5".as_ref()));
}

#[test]
fn test_fast_path_expiration() {
    let cache = FastPathCache::new(1024, 100);

    // Put with short TTL
    let metadata = CacheMetadata {
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: Some(SystemTime::now() - Duration::from_secs(1)), // Already expired
        size_bytes: 5,
        access_count: 0,
        content_hash: String::new(),
        cache_version: 3,
    };

    cache.put_small("expired".to_string(), b"value".to_vec(), metadata);

    // Should return None due to expiration
    assert!(cache.get_small("expired").is_none());
}
