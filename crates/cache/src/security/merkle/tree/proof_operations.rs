//! Proof generation and verification operations

use super::compute_internal_hash;
use crate::errors::{CacheError, RecoveryHint, Result};
use crate::security::merkle::{
    nodes::{hash_to_hex, Hash, MerkleNode},
    proofs::{MerkleProof, ProofStep},
};
use std::collections::HashMap;

/// Compute proof path for a leaf hash
pub fn compute_proof_path(
    nodes: &HashMap<Hash, MerkleNode>,
    leaf_hash: Hash,
) -> Result<Vec<ProofStep>> {
    let mut proof_path = Vec::new();
    let mut current_hash = leaf_hash;

    // Find path from leaf to root
    while let Some(parent_hash) = find_parent(nodes, current_hash)? {
        let parent_node = nodes
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
pub fn find_parent(nodes: &HashMap<Hash, MerkleNode>, child_hash: Hash) -> Result<Option<Hash>> {
    for (parent_hash, parent_node) in nodes {
        if parent_node.left_child == Some(child_hash) || parent_node.right_child == Some(child_hash)
        {
            return Ok(Some(*parent_hash));
        }
    }
    Ok(None)
}

/// Compute root hash from a Merkle proof
pub fn compute_root_from_proof(proof: &MerkleProof) -> Result<Hash> {
    let mut current_hash = proof.entry_hash;

    for step in &proof.proof_path {
        current_hash = if step.is_left_sibling {
            compute_internal_hash(step.sibling_hash, current_hash)
        } else {
            compute_internal_hash(current_hash, step.sibling_hash)
        };
    }

    Ok(current_hash)
}
