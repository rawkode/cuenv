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

pub mod integrity;
pub mod nodes;
pub mod proofs;
pub mod state;
pub mod tree;

// Re-export main types
pub use integrity::IntegrityReport;
pub use nodes::{CacheEntryMetadata, Hash, MerkleNode};
pub use proofs::{MerkleProof, ProofStep};
pub use state::{MerkleTreeState, MerkleTreeStats};
pub use tree::MerkleTree;

// Re-export utility functions

// Test modules
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_proptest;
