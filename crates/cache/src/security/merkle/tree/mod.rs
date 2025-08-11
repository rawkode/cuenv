//! Main Merkle tree implementation and operations

mod hash_computation;
mod operations;
mod proof_operations;
mod structure;
mod verification;

use super::{
    integrity::IntegrityReport,
    nodes::{CacheEntryMetadata, Hash, MerkleNode},
    proofs::MerkleProof,
    state::{MerkleTreeState, MerkleTreeStats},
};
use crate::errors::Result;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

pub(crate) use hash_computation::{compute_internal_hash, compute_leaf_hash};
use operations::{insert_entry_impl, remove_entry_impl};
use proof_operations::{compute_proof_path, compute_root_from_proof};
use structure::rebuild_tree_impl;
use verification::verify_integrity_impl;

/// Production-grade Merkle tree for cache integrity
#[derive(Debug)]
pub struct MerkleTree {
    /// All nodes in the tree indexed by hash
    pub(super) nodes: HashMap<Hash, MerkleNode>,
    /// Leaf nodes indexed by cache key
    pub(super) leaves: BTreeMap<String, Hash>,
    /// Current root hash
    pub(super) root_hash: Option<Hash>,
    /// Tree statistics
    pub(super) stats: MerkleTreeStats,
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
        insert_entry_impl(self, cache_key, content_hash, metadata)
    }

    /// Remove a cache entry from the tree
    pub fn remove_entry(&mut self, cache_key: &str) -> Result<bool> {
        remove_entry_impl(self, cache_key)
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

        let proof_path = compute_proof_path(&self.nodes, leaf_hash)?;
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
        let computed_root = compute_root_from_proof(proof)?;
        Ok(computed_root == proof.root_hash)
    }

    /// Verify the entire tree integrity
    pub fn verify_integrity(&mut self) -> Result<IntegrityReport> {
        verify_integrity_impl(self)
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

    /// Rebuild the tree structure (internal use)
    pub(super) fn rebuild_tree(&mut self) -> Result<()> {
        rebuild_tree_impl(self)
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MerkleTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::security::merkle::nodes::hash_to_hex;

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
