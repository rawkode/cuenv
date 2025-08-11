//! Proof generation and verification tests

use super::helpers::*;
use crate::security::merkle::MerkleTree;
use sha2::{Digest, Sha256};

#[test]
fn test_proof_generation_and_verification() {
    let mut tree = MerkleTree::new();

    // Insert test entries
    for i in 0..8 {
        let key = format!("key_{i}");
        let hash = {
            let mut h = [0u8; 32];
            h[0] = i as u8;
            h
        };
        tree.insert_entry(key, hash, create_test_metadata_with_hash(1024, hash))
            .unwrap();
    }

    // Generate proof for first entry
    let proof = tree.generate_proof("key_0").unwrap();
    assert!(proof.is_some());

    let proof = proof.unwrap();
    assert_eq!(proof.cache_key, "key_0");
    assert_eq!(proof.tree_size, 8);

    // Verify the proof
    assert!(tree.verify_proof(&proof).unwrap());

    // Verify proof for non-existent entry
    let no_proof = tree.generate_proof("nonexistent");
    assert!(no_proof.unwrap().is_none());
}

#[test]
fn test_proof_generation_verification() {
    let mut tree = MerkleTree::new();

    // Insert multiple entries to create a tree with height > 0
    let keys = vec!["ra", "a", "b", "c", "d", "e", "f", "g", "h"];
    for (i, key) in keys.iter().enumerate() {
        let hash = {
            let mut h = [0u8; 32];
            let key_hash = Sha256::digest(key.as_bytes());
            h.copy_from_slice(&key_hash[..32]);
            h
        };
        tree.insert_entry(
            key.to_string(),
            hash,
            create_test_metadata_with_hash((i + 1) as u64, hash),
        )
        .unwrap();
    }

    println!("Tree stats after insertions: {:?}", tree.stats());
    println!("Tree height: {}", tree.stats().height);

    // Test proof generation and verification for each key
    for key in &keys {
        println!("\nTesting proof for key: {key}");

        let proof = tree.generate_proof(key).unwrap();
        assert!(proof.is_some(), "Proof should be generated for key: {key}");

        let proof = proof.unwrap();
        println!("Proof path length: {}", proof.proof_path.len());

        let is_valid = tree.verify_proof(&proof).unwrap();
        assert!(is_valid, "Proof should be valid for key: {key}");
    }
}
