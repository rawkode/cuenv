//! Core tree operations: insert and remove entries

use super::{compute_leaf_hash, MerkleTree};
use crate::errors::Result;
use crate::security::merkle::nodes::{CacheEntryMetadata, Hash, MerkleNode};

/// Insert or update a cache entry in the tree
pub fn insert_entry_impl(
    tree: &mut MerkleTree,
    cache_key: String,
    content_hash: Hash,
    metadata: CacheEntryMetadata,
) -> Result<()> {
    // Create leaf node for the entry
    let leaf_hash = compute_leaf_hash(&cache_key, &content_hash, &metadata);

    let leaf_node = MerkleNode {
        hash: leaf_hash,
        left_child: None,
        right_child: None,
        depth: 0,
        cache_key: Some(cache_key.clone()),
        entry_metadata: Some(metadata),
    };

    // Remove old entry if it exists
    if let Some(old_hash) = tree.leaves.remove(&cache_key) {
        tree.nodes.remove(&old_hash);
        tree.stats.leaf_count -= 1;
    }

    // Insert new leaf
    tree.nodes.insert(leaf_hash, leaf_node);
    tree.leaves.insert(cache_key, leaf_hash);
    tree.stats.leaf_count += 1;

    // Rebuild tree structure
    tree.rebuild_tree()?;

    tree.stats.last_updated = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(())
}

/// Remove a cache entry from the tree
pub fn remove_entry_impl(tree: &mut MerkleTree, cache_key: &str) -> Result<bool> {
    if let Some(leaf_hash) = tree.leaves.remove(cache_key) {
        tree.nodes.remove(&leaf_hash);
        tree.stats.leaf_count -= 1;

        // Rebuild tree structure
        tree.rebuild_tree()?;

        tree.stats.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(true)
    } else {
        Ok(false)
    }
}
