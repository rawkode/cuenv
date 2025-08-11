//! Transaction management for atomic operations
//!
//! This module provides transaction support for grouping multiple
//! cache operations into atomic units.

use super::wal::WalOperation;
use crate::errors::{CacheError, RecoveryHint, Result};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

/// Transaction manager for atomic operations
pub struct TransactionManager {
    /// Active transactions
    transactions: Arc<RwLock<HashMap<u64, Vec<WalOperation>>>>,
    /// Transaction counter
    tx_counter: Arc<Mutex<u64>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            tx_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Begin a new transaction
    pub fn begin(&self) -> u64 {
        let mut counter = self.tx_counter.lock();
        *counter += 1;
        let tx_id = *counter;

        self.transactions.write().insert(tx_id, Vec::new());
        tx_id
    }

    /// Add an operation to a transaction
    pub fn add_operation(&self, tx_id: u64, op: WalOperation) -> Result<()> {
        let mut transactions = self.transactions.write();
        match transactions.get_mut(&tx_id) {
            Some(ops) => {
                ops.push(op);
                Ok(())
            }
            None => Err(CacheError::Configuration {
                message: format!("Transaction {tx_id} not found"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Begin a transaction before adding operations".to_string(),
                },
            }),
        }
    }

    /// Get and remove operations for a transaction
    pub fn take_operations(&self, tx_id: u64) -> Result<Vec<WalOperation>> {
        let mut transactions = self.transactions.write();
        match transactions.remove(&tx_id) {
            Some(ops) => Ok(ops),
            None => Err(CacheError::Configuration {
                message: format!("Transaction {tx_id} not found"),
                recovery_hint: RecoveryHint::Manual {
                    instructions: "Transaction may have already been committed".to_string(),
                },
            }),
        }
    }

    /// Rollback a transaction
    pub fn rollback(&self, tx_id: u64) {
        self.transactions.write().remove(&tx_id);
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
