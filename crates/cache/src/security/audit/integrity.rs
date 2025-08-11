//! Integrity verification and hash chain management

use chrono::Utc;
use sha2::{Digest, Sha256};

use super::types::AuditLogEntry;

/// Compute integrity hash for an entry
pub fn compute_entry_hash(entry: &AuditLogEntry) -> String {
    let mut hasher = Sha256::new();

    // Hash all fields except the integrity_hash itself
    hasher.update(entry.entry_id.as_bytes());
    hasher.update(entry.timestamp.to_rfc3339().as_bytes());
    hasher.update(serde_json::to_vec(&entry.event).unwrap_or_default());
    hasher.update(serde_json::to_vec(&entry.context).unwrap_or_default());
    hasher.update(entry.previous_hash.as_bytes());
    hasher.update(entry.schema_version.to_le_bytes());

    let hash = hasher.finalize();
    hex::encode(hash)
}

/// Compute genesis hash for the first entry
pub fn compute_genesis_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"CUENV_AUDIT_LOG_GENESIS");
    hasher.update(Utc::now().timestamp().to_le_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}
