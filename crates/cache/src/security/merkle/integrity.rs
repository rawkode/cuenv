//! Integrity verification and reporting for the Merkle tree

use super::nodes::Hash;
use serde::{Deserialize, Serialize};

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
