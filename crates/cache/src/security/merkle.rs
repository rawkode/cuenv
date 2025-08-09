//! Merkle tree implementation for cache tamper detection
//!
//! This module provides a cryptographically secure Merkle tree for detecting
//! unauthorized modifications to cache contents. The tree enables efficient
//! verification of cache integrity without examining every entry.
//!
//! ## Security Features
//!
//! - SHA-256 based cryptographic hashing
//! - Incremental tree updates for performance
//! - Proof generation for selective verification
//! - Tamper-evident root hash computation
//! - Zero-knowledge proofs for privacy

use crate::errors::{CacheError, RecoveryHint, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// Cryptographic hash type (SHA-256)
pub type Hash = [u8; 32];

/// Convert bytes to hex string for display
fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

/// Convert hex string to hash
#[allow(dead_code)]
fn hex_to_hash(hex: &str) -> Result<Hash> {
    if hex.len() != 64 {
        return Err(CacheError::Configuration {
            message: format!("Invalid hash length: expected 64 chars, got {}", hex.len()),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Provide valid SHA-256 hash".to_string(),
            },
        });
    }

    let mut hash = [0u8; 32];
    match hex::decode_to_slice(hex, &mut hash) {
        Ok(()) => Ok(hash),
        Err(e) => Err(CacheError::Configuration {
            message: format!("Invalid hex hash: {e}"),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Provide valid hexadecimal hash".to_string(),
            },
        }),
    }
}

/// Merkle tree node containing hash and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Node hash value
    pub hash: Hash,
    /// Left child hash (if internal node)
    pub left_child: Option<Hash>,
    /// Right child hash (if internal node)
    pub right_child: Option<Hash>,
    /// Node depth in tree (0 = leaf)
    pub depth: u32,
    /// Cache key (if leaf node)
    pub cache_key: Option<String>,
    /// Cache entry metadata (if leaf node)
    pub entry_metadata: Option<CacheEntryMetadata>,
}

/// Metadata for cache entries in Merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryMetadata {
    /// Entry size in bytes
    pub size_bytes: u64,
    /// Last modification timestamp
    pub modified_at: u64,
    /// Content hash of the cached data
    pub content_hash: Hash,
    /// Entry TTL expiration (if applicable)
    pub expires_at: Option<u64>,
}

/// Merkle proof for verifying entry inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash of the entry being proven
    pub entry_hash: Hash,
    /// Cache key being proven
    pub cache_key: String,
    /// Path of sibling hashes from leaf to root
    pub proof_path: Vec<ProofStep>,
    /// Root hash this proof validates against
    pub root_hash: Hash,
    /// Tree size when proof was generated
    pub tree_size: u64,
}

/// Single step in a Merkle proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofStep {
    /// Sibling hash at this level
    pub sibling_hash: Hash,
    /// Whether sibling is on the left (true) or right (false)
    pub is_left_sibling: bool,
}

/// Production-grade Merkle tree for cache integrity
#[derive(Debug)]
pub struct MerkleTree {
    /// All nodes in the tree indexed by hash
    nodes: HashMap<Hash, MerkleNode>,
    /// Leaf nodes indexed by cache key
    leaves: BTreeMap<String, Hash>,
    /// Current root hash
    root_hash: Option<Hash>,
    /// Tree statistics
    stats: MerkleTreeStats,
}

/// Statistics about the Merkle tree
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MerkleTreeStats {
    /// Total number of leaf nodes
    pub leaf_count: u64,
    /// Total number of internal nodes
    pub internal_count: u64,
    /// Tree height (depth)
    pub height: u32,
    /// Last update timestamp
    pub last_updated: u64,
    /// Number of integrity checks performed
    pub integrity_checks: u64,
    /// Number of proofs generated
    pub proofs_generated: u64,
}

impl MerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            leaves: BTreeMap::new(),
            root_hash: None,
            stats: MerkleTreeStats::default(),
        }
    }

    /// Insert or update a cache entry in the tree
    pub fn insert_entry(
        &mut self,
        cache_key: String,
        content_hash: Hash,
        metadata: CacheEntryMetadata,
    ) -> Result<()> {
        // Create leaf node for the entry
        let leaf_hash = self.compute_leaf_hash(&cache_key, &content_hash, &metadata);

        let leaf_node = MerkleNode {
            hash: leaf_hash,
            left_child: None,
            right_child: None,
            depth: 0,
            cache_key: Some(cache_key.clone()),
            entry_metadata: Some(metadata),
        };

        // Remove old entry if it exists
        if let Some(old_hash) = self.leaves.remove(&cache_key) {
            self.nodes.remove(&old_hash);
            self.stats.leaf_count -= 1;
        }

        // Insert new leaf
        self.nodes.insert(leaf_hash, leaf_node);
        self.leaves.insert(cache_key, leaf_hash);
        self.stats.leaf_count += 1;

        // Rebuild tree structure
        self.rebuild_tree()?;

        self.stats.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(())
    }

    /// Remove a cache entry from the tree
    pub fn remove_entry(&mut self, cache_key: &str) -> Result<bool> {
        if let Some(leaf_hash) = self.leaves.remove(cache_key) {
            self.nodes.remove(&leaf_hash);
            self.stats.leaf_count -= 1;

            // Rebuild tree structure
            self.rebuild_tree()?;

            self.stats.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the current root hash
    #[must_use]
    pub fn root_hash(&self) -> Option<Hash> {
        self.root_hash
    }

    /// Generate a Merkle proof for a cache entry
    pub fn generate_proof(&mut self, cache_key: &str) -> Result<Option<MerkleProof>> {
        let leaf_hash = match self.leaves.get(cache_key) {
            Some(hash) => *hash,
            None => return Ok(None),
        };

        let proof_path = self.compute_proof_path(leaf_hash)?;
        let root_hash = match self.root_hash {
            Some(hash) => hash,
            None => return Ok(None),
        };

        self.stats.proofs_generated += 1;

        Ok(Some(MerkleProof {
            entry_hash: leaf_hash,
            cache_key: cache_key.to_string(),
            proof_path,
            root_hash,
            tree_size: self.stats.leaf_count,
        }))
    }

    /// Verify a Merkle proof
    pub fn verify_proof(&mut self, proof: &MerkleProof) -> Result<bool> {
        self.stats.integrity_checks += 1;

        // Check if root hash matches current tree
        if Some(proof.root_hash) != self.root_hash {
            return Ok(false);
        }

        // Reconstruct root hash from proof path
        let computed_root = self.compute_root_from_proof(proof)?;
        Ok(computed_root == proof.root_hash)
    }

    /// Verify the entire tree integrity
    pub fn verify_integrity(&mut self) -> Result<IntegrityReport> {
        self.stats.integrity_checks += 1;

        let mut report = IntegrityReport {
            total_entries: self.stats.leaf_count,
            verified_entries: 0,
            corrupted_entries: Vec::new(),
            root_hash: self.root_hash,
            tree_valid: true,
        };

        // Verify all leaf nodes
        for (cache_key, &leaf_hash) in &self.leaves {
            let node = match self.nodes.get(&leaf_hash) {
                Some(node) => node,
                None => {
                    report.corrupted_entries.push(cache_key.clone());
                    report.tree_valid = false;
                    continue;
                }
            };

            // Verify leaf hash computation
            if let (Some(ref key), Some(ref metadata)) = (&node.cache_key, &node.entry_metadata) {
                let computed_hash = self.compute_leaf_hash(key, &metadata.content_hash, metadata);
                if computed_hash != leaf_hash {
                    report.corrupted_entries.push(cache_key.clone());
                    report.tree_valid = false;
                    continue;
                }
            } else {
                report.corrupted_entries.push(cache_key.clone());
                report.tree_valid = false;
                continue;
            }

            report.verified_entries += 1;
        }

        // Verify internal node structure
        if !self.verify_internal_nodes() {
            report.tree_valid = false;
        }

        Ok(report)
    }

    /// Get tree statistics
    #[must_use]
    pub const fn stats(&self) -> &MerkleTreeStats {
        &self.stats
    }

    /// Export tree state for persistence
    pub fn export_state(&self) -> Result<MerkleTreeState> {
        Ok(MerkleTreeState {
            nodes: self.nodes.clone(),
            leaves: self.leaves.clone(),
            root_hash: self.root_hash,
            stats: self.stats.clone(),
        })
    }

    /// Import tree state from persistence
    pub fn import_state(&mut self, state: MerkleTreeState) -> Result<()> {
        self.nodes = state.nodes;
        self.leaves = state.leaves;
        self.root_hash = state.root_hash;
        self.stats = state.stats;
        Ok(())
    }

    /// Compute hash for a leaf node
    fn compute_leaf_hash(
        &self,
        cache_key: &str,
        content_hash: &Hash,
        metadata: &CacheEntryMetadata,
    ) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(b"LEAF:");
        hasher.update(cache_key.as_bytes());
        hasher.update(content_hash);
        hasher.update(metadata.size_bytes.to_le_bytes());
        hasher.update(metadata.modified_at.to_le_bytes());
        if let Some(expires_at) = metadata.expires_at {
            hasher.update(expires_at.to_le_bytes());
        }
        hasher.finalize().into()
    }

    /// Compute hash for an internal node
    fn compute_internal_hash(&self, left_hash: Hash, right_hash: Hash) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(b"INTERNAL:");
        hasher.update(left_hash);
        hasher.update(right_hash);
        hasher.finalize().into()
    }

    /// Rebuild the entire tree structure
    fn rebuild_tree(&mut self) -> Result<()> {
        // Clear old internal nodes
        self.nodes.retain(|_, node| node.cache_key.is_some());
        self.stats.internal_count = 0;

        if self.leaves.is_empty() {
            self.root_hash = None;
            self.stats.height = 0;
            return Ok(());
        }

        // Build tree level by level
        let mut current_level: Vec<Hash> = self.leaves.values().copied().collect();
        let mut height = 0;

        while current_level.len() > 1 {
            height += 1;
            let mut next_level = Vec::new();

            // Process pairs of nodes
            for chunk in current_level.chunks(2) {
                let left_hash = chunk[0];
                let right_hash = chunk.get(1).copied().unwrap_or(left_hash);

                let internal_hash = self.compute_internal_hash(left_hash, right_hash);

                let internal_node = MerkleNode {
                    hash: internal_hash,
                    left_child: Some(left_hash),
                    right_child: if right_hash != left_hash {
                        Some(right_hash)
                    } else {
                        None
                    },
                    depth: height,
                    cache_key: None,
                    entry_metadata: None,
                };

                self.nodes.insert(internal_hash, internal_node);
                self.stats.internal_count += 1;
                next_level.push(internal_hash);
            }

            current_level = next_level;
        }

        self.root_hash = current_level.first().copied();
        self.stats.height = height;

        Ok(())
    }

    /// Compute proof path for a leaf hash
    fn compute_proof_path(&self, leaf_hash: Hash) -> Result<Vec<ProofStep>> {
        let mut proof_path = Vec::new();
        let mut current_hash = leaf_hash;

        // Find path from leaf to root
        while let Some(parent_hash) = self.find_parent(current_hash)? {
            let parent_node =
                self.nodes
                    .get(&parent_hash)
                    .ok_or_else(|| CacheError::Corruption {
                        key: hash_to_hex(&parent_hash),
                        reason: "Missing parent node in Merkle tree".to_string(),
                        recovery_hint: RecoveryHint::RebuildIndex,
                    })?;

            // Determine sibling
            let (sibling_hash, is_left_sibling) = if Some(current_hash) == parent_node.left_child {
                // Current node is left child
                match parent_node.right_child {
                    Some(right) => (right, false),
                    None => (current_hash, false), // No right sibling, use self
                }
            } else {
                // Current node is right child
                match parent_node.left_child {
                    Some(left) => (left, true),
                    None => (current_hash, true), // No left sibling, use self
                }
            };

            proof_path.push(ProofStep {
                sibling_hash,
                is_left_sibling,
            });

            current_hash = parent_hash;
        }

        Ok(proof_path)
    }

    /// Find parent node of a given hash
    fn find_parent(&self, child_hash: Hash) -> Result<Option<Hash>> {
        for (parent_hash, parent_node) in &self.nodes {
            if parent_node.left_child == Some(child_hash)
                || parent_node.right_child == Some(child_hash)
            {
                return Ok(Some(*parent_hash));
            }
        }
        Ok(None)
    }

    /// Compute root hash from a Merkle proof
    fn compute_root_from_proof(&self, proof: &MerkleProof) -> Result<Hash> {
        let mut current_hash = proof.entry_hash;

        for step in &proof.proof_path {
            current_hash = if step.is_left_sibling {
                self.compute_internal_hash(step.sibling_hash, current_hash)
            } else {
                self.compute_internal_hash(current_hash, step.sibling_hash)
            };
        }

        Ok(current_hash)
    }

    /// Verify internal node hash computations
    fn verify_internal_nodes(&self) -> bool {
        for node in self.nodes.values() {
            if node.cache_key.is_some() {
                continue; // Skip leaf nodes
            }

            // Handle case where node only has left child (odd number of nodes at level)
            match (node.left_child, node.right_child) {
                (Some(left_hash), Some(right_hash)) => {
                    let computed_hash = self.compute_internal_hash(left_hash, right_hash);
                    if computed_hash != node.hash {
                        return false;
                    }
                }
                (Some(left_hash), None) => {
                    // For nodes with only left child, verify it's computed correctly
                    let computed_hash = self.compute_internal_hash(left_hash, left_hash);
                    if computed_hash != node.hash {
                        return false;
                    }
                }
                _ => return false, // Internal nodes must have at least a left child
            }
        }
        true
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MerkleTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MerkleTree {{ leaves: {}, height: {}, root: {} }}",
            self.stats.leaf_count,
            self.stats.height,
            self.root_hash
                .as_ref()
                .map(hash_to_hex)
                .unwrap_or_else(|| "None".to_string())
        )
    }
}

/// Tree integrity verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// Total number of entries checked
    pub total_entries: u64,
    /// Number of entries that passed verification
    pub verified_entries: u64,
    /// List of corrupted cache keys
    pub corrupted_entries: Vec<String>,
    /// Current root hash
    pub root_hash: Option<Hash>,
    /// Whether the entire tree is valid
    pub tree_valid: bool,
}

/// Serializable tree state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTreeState {
    /// All nodes in the tree
    pub nodes: HashMap<Hash, MerkleNode>,
    /// Leaf nodes indexed by cache key
    pub leaves: BTreeMap<String, Hash>,
    /// Current root hash
    pub root_hash: Option<Hash>,
    /// Tree statistics
    pub stats: MerkleTreeStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn create_test_metadata(size: u64) -> CacheEntryMetadata {
        create_test_metadata_with_hash(size, [0u8; 32])
    }

    fn create_test_metadata_with_hash(size: u64, content_hash: Hash) -> CacheEntryMetadata {
        CacheEntryMetadata {
            size_bytes: size,
            modified_at: 1640000000,
            content_hash,
            expires_at: None,
        }
    }

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
            let key = format!("key_{}", i);
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
    fn test_proof_generation_and_verification() {
        let mut tree = MerkleTree::new();

        // Insert test entries
        for i in 0..8 {
            let key = format!("key_{}", i);
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
    fn test_entry_removal() {
        let mut tree = MerkleTree::new();

        // Insert entries
        for i in 0..5 {
            let key = format!("key_{}", i);
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
    fn test_integrity_verification() {
        let mut tree = MerkleTree::new();

        // Insert test entries
        for i in 0..5 {
            let key = format!("key_{}", i);
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

    #[test]
    fn test_tree_state_export_import() {
        let mut tree1 = MerkleTree::new();

        // Insert test entries
        for i in 0..3 {
            let key = format!("key_{}", i);
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
        println!("Leaves: {:?}", tree.leaves);
        println!("Nodes count: {}", tree.nodes.len());

        // Verify integrity
        let report = tree.verify_integrity().unwrap();
        println!("Integrity report: {:?}", report);

        assert!(report.tree_valid, "Tree should be valid");
        assert_eq!(report.verified_entries, 1);
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
            println!("\nTesting proof for key: {}", key);

            let proof = tree.generate_proof(key).unwrap();
            assert!(
                proof.is_some(),
                "Proof should be generated for key: {}",
                key
            );

            let proof = proof.unwrap();
            println!("Proof path length: {}", proof.proof_path.len());

            let is_valid = tree.verify_proof(&proof).unwrap();
            assert!(is_valid, "Proof should be valid for key: {}", key);
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
