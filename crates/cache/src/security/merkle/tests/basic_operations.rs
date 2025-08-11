//! Basic tree operation tests

use super::helpers::*;
use crate::security::merkle::MerkleTree;
use sha2::{Digest, Sha256};

#[test]
fn test_empty_tree() {
    let tree = MerkleTree::new();
    assert_eq!(tree.root_hash(), None);
    assert_eq!(tree.stats().leaf_count, 0);
    assert_eq!(tree.stats().height, 0);
}

#[test]
fn test_single_entry() {
    let mut tree = MerkleTree::new();
    let content_hash = [1u8; 32];
    let metadata = create_test_metadata_with_hash(1024, content_hash);

    tree.insert_entry("test/key".to_string(), content_hash, metadata)
        .unwrap();

    assert!(tree.root_hash().is_some());
    assert_eq!(tree.stats().leaf_count, 1);
    assert_eq!(tree.stats().height, 0);
}

#[test]
fn test_multiple_entries() {
    let mut tree = MerkleTree::new();

    for i in 0..10 {
        let key = format!("key_{i}");
        let hash = {
            let mut h = [0u8; 32];
            h[0] = i as u8;
            h
        };
        tree.insert_entry(key, hash, create_test_metadata_with_hash(1024, hash))
            .unwrap();
    }

    assert!(tree.root_hash().is_some());
    assert_eq!(tree.stats().leaf_count, 10);
    assert!(tree.stats().height > 0);
}

#[test]
fn test_entry_removal() {
    let mut tree = MerkleTree::new();

    // Insert entries
    for i in 0..5 {
        let key = format!("key_{i}");
        let hash = {
            let mut h = [0u8; 32];
            h[0] = i as u8;
            h
        };
        tree.insert_entry(key, hash, create_test_metadata_with_hash(1024, hash))
            .unwrap();
    }

    let original_root = tree.root_hash();
    assert_eq!(tree.stats().leaf_count, 5);

    // Remove an entry
    assert!(tree.remove_entry("key_2").unwrap());
    assert_eq!(tree.stats().leaf_count, 4);

    // Root should change
    assert_ne!(tree.root_hash(), original_root);

    // Removing non-existent entry should return false
    assert!(!tree.remove_entry("nonexistent").unwrap());
}

#[test]
fn test_simple_tree_operation() {
    let mut tree = MerkleTree::new();

    // Test with single entry
    let key = "a";
    let size = 1;
    let hash = {
        let mut h = [0u8; 32];
        let key_hash = Sha256::digest(key.as_bytes());
        h.copy_from_slice(&key_hash[..32]);
        h
    };

    tree.insert_entry(
        key.to_string(),
        hash,
        create_test_metadata_with_hash(size, hash),
    )
    .unwrap();

    // Debug print tree state
    println!("Tree stats: {:?}", tree.stats());
    println!("Root hash: {:?}", tree.root_hash());

    // Verify integrity
    let report = tree.verify_integrity().unwrap();
    println!("Integrity report: {report:?}");

    assert!(report.tree_valid, "Tree should be valid");
    assert_eq!(report.verified_entries, 1);
}
