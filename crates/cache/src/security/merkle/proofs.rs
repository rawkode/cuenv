//! Merkle proof generation and verification

use super::nodes::Hash;
use serde::{Deserialize, Serialize};

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
