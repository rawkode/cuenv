//! Test to check serialization behavior
#[cfg(test)]
mod serialization_tests {
    #[test]
    fn test_byte_slice_serialization() {
        let value = b"recovery_test";

        // Test bincode serialization
        let serialized = bincode::serialize(&value).unwrap();
        println!(
            "Serialized b\"recovery_test\": {:?} (len: {})",
            serialized,
            serialized.len()
        );

        // Deserialize back
        let deserialized: Vec<u8> = bincode::deserialize(&serialized).unwrap();
        println!("Deserialized: {:?}", deserialized);

        assert_eq!(deserialized, value.to_vec());
    }

    #[test]
    fn test_empty_serialization() {
        let empty: Vec<u8> = vec![];
        let serialized = bincode::serialize(&empty).unwrap();
        println!(
            "Serialized empty vec: {:?} (len: {})",
            serialized,
            serialized.len()
        );

        // Try to deserialize empty data
        let empty_data: &[u8] = &[];
        let result: Result<Vec<u8>, _> = bincode::deserialize(empty_data);
        println!("Deserializing empty data: {:?}", result);
    }
}
