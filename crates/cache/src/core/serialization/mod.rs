//! Core cache serialization utilities

use crate::errors::{CacheError, Result, SerializationOp, RecoveryHint};
use serde::{de::DeserializeOwned, Serialize};

/// Serialize a value to bytes for cache storage
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    bincode::serialize(value).map_err(|e| {
        CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Encode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::Manual {
                instructions: "Check that the value is serializable".to_string(),
            },
        }
    })
}

/// Deserialize bytes to a value from cache storage
pub fn deserialize<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
    bincode::deserialize(data).map_err(|e| {
        CacheError::Serialization {
            key: String::new(),
            operation: SerializationOp::Decode,
            source: Box::new(e),
            recovery_hint: RecoveryHint::ClearAndRetry,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_serialize_deserialize() {
        let original = TestData {
            id: 42,
            name: "test".to_string(),
            values: vec![1, 2, 3],
        };

        let serialized = serialize(&original).unwrap();
        let deserialized: TestData = deserialize(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_deserialize_invalid_data() {
        let invalid_data = b"invalid data";
        let result: Result<TestData> = deserialize(invalid_data);
        assert!(result.is_err());
    }
}