//! State export/import tests

use super::helpers::*;
use crate::security::merkle::MerkleTree;

#[test]
fn test_tree_state_export_import() {
    let mut tree1 = MerkleTree::new();

    // Insert test entries
    for i in 0..3 {
        let key = format!("key_{i}");
        let hash = {
            let mut h = [0u8; 32];
            h[0] = i as u8;
            h
        };
        tree1
            .insert_entry(key, hash, create_test_metadata_with_hash(1024, hash))
            .unwrap();
    }

    let original_root = tree1.root_hash();
    let state = tree1.export_state().unwrap();

    // Create new tree and import state
    let mut tree2 = MerkleTree::new();
    tree2.import_state(state).unwrap();

    assert_eq!(tree2.root_hash(), original_root);
    assert_eq!(tree2.stats().leaf_count, 3);
}
