//! Property-based tests for the Merkle tree

#[cfg(test)]
mod proptest_tests {
    use crate::security::merkle::{CacheEntryMetadata, Hash, MerkleTree};
    use proptest::prelude::*;
    use sha2::{Digest, Sha256};

    fn create_test_metadata_with_hash(size: u64, content_hash: Hash) -> CacheEntryMetadata {
        CacheEntryMetadata {
            size_bytes: size,
            modified_at: 1640000000,
            content_hash,
            expires_at: None,
        }
    }

    proptest! {
        #[test]
        fn test_tree_operations(
            keys in prop::collection::vec("[a-z]{1,10}", 1..20),
            sizes in prop::collection::vec(1u64..10000, 1..20)
        ) {
            let mut tree = MerkleTree::new();
            let mut unique_keys = std::collections::HashSet::new();

            // Insert entries
            for (key, size) in keys.iter().zip(sizes.iter()) {
                let hash = {
                    let mut h = [0u8; 32];
                    // Use key hash for deterministic content hash
                    let key_hash = Sha256::digest(key.as_bytes());
                    h.copy_from_slice(&key_hash[..32]);
                    h
                };
                tree.insert_entry(key.clone(), hash, create_test_metadata_with_hash(*size, hash)).unwrap();
                unique_keys.insert(key.clone());
            }

            // Verify tree has correct number of unique entries
            prop_assert_eq!(tree.stats().leaf_count, unique_keys.len() as u64);

            if !unique_keys.is_empty() {
                prop_assert!(tree.root_hash().is_some());

                // Generate and verify proof for first key
                if let Some(proof) = tree.generate_proof(&keys[0]).unwrap() {
                    prop_assert!(tree.verify_proof(&proof).unwrap());
                }

                // Verify integrity
                let report = tree.verify_integrity().unwrap();
                prop_assert!(report.tree_valid);
                prop_assert_eq!(report.verified_entries, unique_keys.len() as u64);
            }
        }
    }
}
