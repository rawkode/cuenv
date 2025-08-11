//! Integrity verification operations

use super::{compute_internal_hash, compute_leaf_hash, MerkleTree};
use crate::errors::Result;
use crate::security::merkle::integrity::IntegrityReport;

/// Verify the entire tree integrity
pub fn verify_integrity_impl(tree: &mut MerkleTree) -> Result<IntegrityReport> {
    tree.stats.integrity_checks += 1;

    let mut report = IntegrityReport {
        total_entries: tree.stats.leaf_count,
        verified_entries: 0,
        corrupted_entries: Vec::new(),
        root_hash: tree.root_hash,
        tree_valid: true,
    };

    // Verify all leaf nodes
    for (cache_key, &leaf_hash) in &tree.leaves {
        let node = match tree.nodes.get(&leaf_hash) {
            Some(node) => node,
            None => {
                report.corrupted_entries.push(cache_key.clone());
                report.tree_valid = false;
                continue;
            }
        };

        // Verify leaf hash computation
        if let (Some(ref key), Some(ref metadata)) = (&node.cache_key, &node.entry_metadata) {
            let computed_hash = compute_leaf_hash(key, &metadata.content_hash, metadata);
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
    if !verify_internal_nodes(tree) {
        report.tree_valid = false;
    }

    Ok(report)
}

/// Verify internal node hash computations
pub fn verify_internal_nodes(tree: &MerkleTree) -> bool {
    for node in tree.nodes.values() {
        if node.cache_key.is_some() {
            continue; // Skip leaf nodes
        }

        // Handle case where node only has left child (odd number of nodes at level)
        match (node.left_child, node.right_child) {
            (Some(left_hash), Some(right_hash)) => {
                let computed_hash = compute_internal_hash(left_hash, right_hash);
                if computed_hash != node.hash {
                    return false;
                }
            }
            (Some(left_hash), None) => {
                // For nodes with only left child, verify it's computed correctly
                let computed_hash = compute_internal_hash(left_hash, left_hash);
                if computed_hash != node.hash {
                    return false;
                }
            }
            _ => return false, // Internal nodes must have at least a left child
        }
    }
    true
}
