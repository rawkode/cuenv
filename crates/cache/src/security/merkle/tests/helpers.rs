//! Test helper functions

use crate::security::merkle::{CacheEntryMetadata, Hash};

pub fn create_test_metadata(size: u64) -> CacheEntryMetadata {
    create_test_metadata_with_hash(size, [0u8; 32])
}

pub fn create_test_metadata_with_hash(size: u64, content_hash: Hash) -> CacheEntryMetadata {
    CacheEntryMetadata {
        size_bytes: size,
        modified_at: 1640000000,
        content_hash,
        expires_at: None,
    }
}
