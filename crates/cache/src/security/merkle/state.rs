//! State management and persistence for the Merkle tree

use super::nodes::{Hash, MerkleNode};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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
