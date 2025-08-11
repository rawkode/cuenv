//! Tree structure management and rebuilding

use super::{compute_internal_hash, MerkleTree};
use crate::errors::Result;
use crate::security::merkle::nodes::{Hash, MerkleNode};

/// Rebuild the entire tree structure
pub fn rebuild_tree_impl(tree: &mut MerkleTree) -> Result<()> {
    // Clear old internal nodes
    tree.nodes.retain(|_, node| node.cache_key.is_some());
    tree.stats.internal_count = 0;

    if tree.leaves.is_empty() {
        tree.root_hash = None;
        tree.stats.height = 0;
        return Ok(());
    }

    // Build tree level by level
    let mut current_level: Vec<Hash> = tree.leaves.values().copied().collect();
    let mut height = 0;

    while current_level.len() > 1 {
        height += 1;
        let mut next_level = Vec::new();

        // Process pairs of nodes
        for chunk in current_level.chunks(2) {
            let left_hash = chunk[0];
            let right_hash = chunk.get(1).copied().unwrap_or(left_hash);

            let internal_hash = compute_internal_hash(left_hash, right_hash);

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

            tree.nodes.insert(internal_hash, internal_node);
            tree.stats.internal_count += 1;
            next_level.push(internal_hash);
        }

        current_level = next_level;
    }

    tree.root_hash = current_level.first().copied();
    tree.stats.height = height;

    Ok(())
}
