//! Integrity verification tests

use super::helpers::*;
use crate::security::merkle::MerkleTree;

#[test]
fn test_integrity_verification() {
    let mut tree = MerkleTree::new();

    // Insert test entries
    for i in 0..5 {
        let key = format!("key_{i}");
        let hash = {
            let mut h = [0u8; 32];
            h[0] = i as u8;
            h
        };
        let mut metadata = create_test_metadata(1024);
        metadata.content_hash = hash; // Make sure metadata matches the content hash
        tree.insert_entry(key, hash, metadata).unwrap();
    }

    let report = tree.verify_integrity().unwrap();
    assert_eq!(report.total_entries, 5);
    assert_eq!(report.verified_entries, 5);
    assert!(report.corrupted_entries.is_empty());
    assert!(report.tree_valid);
}
